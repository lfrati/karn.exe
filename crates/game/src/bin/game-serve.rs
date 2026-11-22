//! The engine behind a pipe: one game at a time, set up by card name, told moves and asked
//! what it is and what may be done, as lines of JSON. Player clients, rule witnesses,
//! and recorded games use this native interface.
//!
//! `cargo run --profile fast -p game --bin game-serve`
//!
//! Player requests (API `player/1`, `game::prompt`): what a program playing a seat sends.
//! - `{"cmd":"prompt","seat":"p0"}`: the game as that seat may see it, labelled (`p1`,
//!   `o12`, `o12.a0`), with each object's text and activated abilities, and the question if
//!   it is this seat's, with who chooses, why, and for which spell or ability. Moves are not
//!   listed, nor searched for. Answered `{"ok":true,"prompt":{…}}`.
//! - `{"cmd":"answer","seat":"p0","session_id":1,"request_id":9,"answer":{"kind":"cast",
//!   "card":"o4","targets":[["p1"]]}}`: a move as `act` takes it, naming by the seat's
//!   labels; answered with that seat's next prompt. Invalid answers are refused without
//!   selecting a replacement action. A player's illegal total combat damage assignment
//!   returns to the moment before they began to assign (510.1e), under a new `request_id`.
//!
//! Every other request is privileged tooling. The pipe trusts its caller: `seat` and
//! `viewer` select a view and authenticate no one. A host seating untrusted players exposes
//! only `prompt` and `answer` for their own seats.
//!
//! Privileged requests, one per line:
//! - `{"cmd":"new","players":[{"hand":[…],"battlefield":[…],"battlefield_tapped":[…],
//!   "graveyard":[…],"exile":[…],"library":[…]},…]}`: card names; of a library the last
//!   name is on top. The game begins at the untap step of the first player's first turn.
//! - `{"cmd":"start","decks":["decks/lands.dck",["Forest","Grizzly Bears",…]],"seed":7,
//!   "first":0}`: a game from its start (CR 103): each deck, the path of a deck file or a
//!   list of card names, is shuffled; each player draws seven and is asked about mulligans
//!   (`"asked":{"kind":"mulligan"}`, answered `{"kind":"mulligan"}` or `{"kind":"keep"}`; then
//!   `{"kind":"bottom","among":[…],"count":1}`, answered `{"kind":"bottom","cards":[…]}`).
//!   In place of `seed`, `"orders":[[2,0,1,…],[…]]` replays a game played elsewhere: for each
//!   player in deck order, the library order the initial shuffle of that deck produced, top
//!   first, as 0-based positions into that player's deck list (for `"format":"commander"`, the
//!   list without the commander); no random number is drawn. `orders` with `seed` is refused.
//!   Until the first turn begins (103.8) the state's `"begun"` is false.
//!   `new` and `start` take `"ask_forced":true`: a question the rules have a player answer
//!   is asked even when one answer is possible (a creature put onto the battlefield
//!   attacking with one possible defender is asked `{"kind":"defender",…}`, 508.4).
//! - `"move":{"kind":"concede"}`: the player who sends it concedes (104.3a), whoever the
//!   request is for and whatever it asks: they lose and leave the game, logged as the events
//!   `conceded` and `lost`. A two-player game ends with the other player as `winner`.
//! - `{"cmd":"act","session_id":1,"request_id":1,"player":0,"move":{"kind":"pass"}}`: see [`read_move`] for the moves.
//!   A move saying a key its kind does not take is refused ([`move_keys`]).
//! - `{"cmd":"state"}`
//! - `{"cmd":"audit_objects","session_id":1}`: privileged effective keywords, and each
//!   stack ability's source and the text it was put on the stack with (405.4, 113.7a),
//!   computed only on request.
//! - `{"cmd":"audit_game","session_id":1}`: privileged retained engine facts ([`Game::audit_game_objects`]).
//!   Each object has its effective `properties` (with `supertypes`), `keywords` (bare names)
//!   and `"held"`: its keyword abilities after continuous effects as their text says them,
//!   parameter and all (`["first strike","ward {2}","equip {3}"]`); `"abilities_removed"`:
//!   an effect removed its rules-text abilities; and `"base"`, its copiable values before
//!   continuous effects (707.2: the face up, a token's creating definition, or copied values):
//!   `{"name","colors","supertypes","types","subtypes","power","toughness","keywords","rows"}`,
//!   `rows` its rules text rows as `definition_rows` prints them. `"combat"` adds
//!   `"damage_steps"` (combat damage steps begun this combat) and `"first_strikers"` (null,
//!   or the creatures with first or double strike as the first one began), as 510.4 says.
//! - `{"cmd":"library_order","session_id":1,"player":0}`: privileged full library
//!   observation, `{"player":0,"order":[...],"identities":{"object_id":physical_id}}`,
//!   top first. Physical identities follow cards across incarnation changes. Neither
//!   full order nor this identity map is part of a seat prompt.
//! - `{"cmd":"set_library_order","session_id":1,"request_id":1,"player":0,"order":[...]}`:
//!   privileged fixture edit at priority. Requires an exact permutation of current IDs,
//!   top first, and issues a fresh request ID. Does not generate a shuffle event.
//! - `act` optionally takes `"random_outcomes":[{"kind":"shuffle","player":0,"order":[...]}]`
//!   or selected-card `{"kind":"random_order","order":[...]}` keyed by membership.
//!   A called coin flip asks `coin_call`, answered with `{"kind":"choose","answer":{"call":"heads"}}`
//!   (or `"tails"`) before consuming `{"kind":"coin","player":0,"result":"heads"}`.
//!   Coin capture returns the same physical result shape; the completed event retains the call.
//!   Orders are nearest the top first; handles are current at response start. Capture uses
//!   `physical_order` instead, with actual owner/destination provenance.
//!   `random_outcomes_keep_unused` returns `random_outcomes_unused` plus original
//!   `random_outcomes_unused_indices` for exact occurrence correlation.
//!   Bad receipts return `code:"replay_input"` and restore the pending request/state.
//! - `act` optionally takes `"shuffles":[{"player":0,"order":[...]}]`: exact outcomes
//!   consumed at actual shuffles during that response, before later draws. IDs name
//!   current physical cards; zone changes during resolution are tracked. An explicit
//!   empty list forbids shuffles. Missing, unused or mismatched outcomes fail replay.
//!   With `"shuffles_keep_unused":true` (only beside `shuffles`), outcomes no shuffle of
//!   the response consumed are answered as `"shuffles_unused":[{"player":0,"order":[...]}]`,
//!   in supplied order, naming each card by its current object ID, top first, instead of
//!   failing. A shuffle with no outcome left, or a mismatched one, still fails replay.
//! - `{"cmd":"version"}`: which engine this is, also said in the `ready` line: `{"ok":true,
//!   "version":{"package":"0.1.0","source":{"revision":"…","dirty":false},"apis":{"harness":
//!   "harness/1","player":"player/1"},"random":"splitmix64-fisher-yates/1"}}`. `source` is what
//!   the builder said it built (`GAME_SOURCE_REVISION`, `GAME_SOURCE_DIRTY`, set by
//!   `scripts/rust_tools.py build`), `null` when it said nothing: never guessed from a
//!   checkout. The executable's own hash is the host's to take.
//! - `{"cmd":"labels","seat":"p0"}`: which object each label given to that seat names,
//!   `{"ok":true,"labels":{"o1":4294967301,…}}`, ids as `state` says them. For a harness
//!   that checks what a seat was shown or answered against the whole game; it names
//!   nothing new and changes nothing, and no player is given it.
//! Each is answered with `{"ok":true,"state":{…}}` or `{"ok":false,"error":"…","class":"…"}`.
//!
//! A request not carried out is refused with what it says (`error`) and what kind of
//! refusal it is (`class`, [`Class`]), the same through `--host`:
//! - `"illegal"`: the rules forbid it, with `"rules":["601.2f",…]`, the rules its `error`
//!   cites (none, `[]`, when it cites none);
//! - `"unsupported"`: the engine does not run something it needs; the game is as it was;
//! - `"malformed"`: the request is not JSON, misses a field or gives one a value it does not
//!   take, says a field its command or move does not take ([`move_keys`]),
//!   describes no game (a setup with one deck), or breaks the protocol (a privileged command
//!   with a `viewer`, replay receipts that do not fit: these also say `"code":"replay_input"`);
//! - `"unreferable"`: it names an object, player, label, request or game the requester
//!   cannot refer to: unknown, hidden from that seat, or not theirs (another player's request);
//! - `"stale"`: it answers a question no longer asked (the game is over, there is none) or
//!   uses an outdated `session_id`, `request_id` or quote.
//!
//! A setup the rules forbid (a Commander deck of 99 cards, 903.5a) is illegal, one the engine
//! does not carry out unsupported. A request is read whole, and the objects and players it
//! names are checked to be the game's, before the game is given it: one refused as
//! malformed, unreferable or stale changes nothing and is not consumed. A move the game
//! refuses leaves it as it was, but where the rules begin a choice again (510.1e).
//! `quote_payment`'s proof says the quoted move's refusal as `error`, its class as
//! `error_class`, and of an illegal one the rules it cites as `error_rules`.
//! A failure of the engine is no refusal and says no class:
//! `{"ok":false,"error":"simulation fault","fault":{…}}`, after which the game cannot go on,
//! or `"simulation setup fault"` for a game that could not be set up.
//!
//! During turn control (723), `asked.player` is the decision-maker and `asked.acting` is
//! the affected rules actor. `acting` is omitted when equal. Each affected player also has
//! `decision_maker` in the snapshot. Both seats may see the affected player's private
//! information and choices; only the decision-maker may answer, using the actor's resources.
//!
//! Any request may say whom its answer is shown to: `"viewer":0`, or `"viewer":"asked"` for
//! the player the game is waiting for, which is how a program playing a seat asks. That player
//! is shown what they may see (CR 400.2, 402.3): their own hand, how many cards the other
//! hands and the libraries have, what is asked only when it is asked of them, and in
//! `"events"` what happened since they last looked, a card they could not see left null; and
//! in `"observations"` the cards they were shown since they last looked, as those were then:
//! `{"seq":3,"kind":"looked","by":0,"cards":[{"object":7,"name":"Forest","owner":0,"zone":"library"}]}`
//! (`"revealed"` for cards shown to every player). They are given once, with or without a
//! question following, and are no permission to see those cards where they are now. With
//! no viewer the answer is the whole game, for a harness that sets games up and compares
//! them; it takes no seat's observations.

#![forbid(unsafe_code)]

use game::card::{CardDef, CastAs};
use game::legal::{Asked, Slot};
#[cfg(test)]
use game::legal::Step;
use game::observation::{Disclosure, Kind};
use game::play::check;
use game::prompt::Label;
use game::simulation::{Class, Fault, RequestId, ResponseError, SetupError, Simulation};
use game::snapshot::Snapshot;
#[cfg(test)]
use game::state::Decision;
use game::state::{Answer, Pool};
use game::{Event, Game, Move, ObjId, Target, Zone};
use ir::mana::ManaSymbol;
use oracle::corpus;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};

/// A request the pipe does not carry out: refused, saying what kind of refusal it is, or
/// failed in the engine (a fault, which is no refusal). What the pipe's readers say of a
/// request they cannot read is a malformed one's refusal; every other is said by its class.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Error { Refused(Class, String), Fault(String) }

impl Error {
    fn illegal(why: impl Into<String>) -> Self { Self::Refused(Class::Illegal, why.into()) }
    fn unsupported(why: impl Into<String>) -> Self { Self::Refused(Class::Unsupported, why.into()) }
    fn unreferable(why: impl Into<String>) -> Self { Self::Refused(Class::Unreferable, why.into()) }
    fn stale(why: impl Into<String>) -> Self { Self::Refused(Class::Stale, why.into()) }
    fn message(&self) -> &str { match self { Self::Refused(_, why) | Self::Fault(why) => why } }

    /// `{"ok":false,…}` as this is said, but for a fault of the game itself, which says the
    /// fault's boundary instead ([`faulted`]).
    fn said(&self) -> Value {
        match self {
            Self::Fault(why) => json!({ "ok": false, "error": why }),
            Self::Refused(class, why) => {
                let mut said = json!({ "ok": false, "error": why, "class": class.name() });
                if *class == Class::Illegal { said["rules"] = json!(game::simulation::cited_rules(why)); }
                if why.starts_with("replay input: ") { said["code"] = json!("replay_input"); }
                said
            }
        }
    }
}

/// A failed game's answer to every request. Panic diagnostics may include hidden
/// information; the host's panic hook reports them on stderr, and this says no card state.
fn faulted(session_id: u64, fault: &Fault) -> Value {
    json!({ "ok": false, "error": "simulation fault", "fault": {
        "session_id": session_id, "request_id": fault.request_id.0, "stage": format!("{:?}", fault.stage),
    }})
}

impl From<String> for Error { fn from(why: String) -> Self { Self::Refused(Class::Malformed, why) } }
impl From<&str> for Error { fn from(why: &str) -> Self { Self::Refused(Class::Malformed, why.into()) } }
impl From<Fault> for Error { fn from(fault: Fault) -> Self { Self::Fault(fault.to_string()) } }
/// A game not set up says why, as the game said what kind of refusal that is.
impl From<game::start::Refused> for Error { fn from(refused: game::start::Refused) -> Self { Self::Refused(refused.class, refused.why) } }
impl From<ResponseError> for Error {
    fn from(error: ResponseError) -> Self {
        match (error.class(), error) {
            (_, ResponseError::Fault(fault)) => fault.into(),
            // a harness names objects by id: one that is gone, or never was
            (_, ResponseError::Unreferable(Target::Object(_))) => Self::unreferable("an object handle is stale or unknown"),
            (Some(class), error) => Self::Refused(class, error.to_string()),
            (None, error) => Self::Fault(error.to_string()),
        }
    }
}
/// What a refusal says, for the pipe's own tools (replay, benchmarks), which report it.
impl From<Error> for String { fn from(error: Error) -> Self { match error { Error::Refused(_, why) | Error::Fault(why) => why } } }
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.message()) }
}
#[cfg(test)]
impl Error {
    fn class(&self) -> Option<Class> { match self { Self::Refused(class, _) => Some(*class), Self::Fault(_) => None } }
}
/// A test reads what a refusal says as it reads any text.
#[cfg(test)]
impl std::ops::Deref for Error { type Target = str; fn deref(&self) -> &str { self.message() } }
#[cfg(test)]
impl PartialEq<&str> for Error { fn eq(&self, other: &&str) -> bool { self.message() == *other } }
#[cfg(test)]
impl PartialEq<String> for Error { fn eq(&self, other: &String) -> bool { self.message() == other } }

thread_local! {
    /// Whether what is said names objects and players by their player-facing labels
    /// (`game::prompt`): while the `prompt` and `answer` commands are said and read.
    static LABELS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// What stands for an object in what is said with labels until it is told to a seat, which
/// names it by that seat's own labels ([`name_objects`]): a seat's names are given in the
/// order it is shown objects, so an object is named only when it is shown.
const UNNAMED: char = '\u{1}';

/// Say (or read) whatever `run` does with labels: `"o12"` for an object (once named for its
/// seat), `"p1"` for a player where a player is a target or an owner, controller or chooser.
fn labelled<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore { fn drop(&mut self) { LABELS.with(|l| l.set(self.0)); } }
    let _restore = Restore(LABELS.with(|l| l.replace(true)));
    run()
}

fn labels() -> bool { LABELS.with(|l| l.get()) }

/// An object as the pipe names it: its number, or (to be named) its label.
fn oid(o: ObjId) -> Value {
    if labels() { json!(key(o)) } else { json!(o.0) }
}

/// An object as a key of a map.
fn key(o: ObjId) -> String {
    if labels() { format!("{UNNAMED}{}", o.0) } else { o.0.to_string() }
}

/// One of an object's abilities, by its place among them: that index, or its label.
fn aid(o: ObjId, index: usize) -> Value {
    if labels() { json!(format!("{UNNAMED}{}.a{index}", o.0)) } else { json!(index) }
}

/// An object (or its ability) that stands to be named, from what [`key`] or [`aid`] said.
fn unnamed(text: &str) -> Option<(ObjId, Option<usize>)> {
    let rest = text.strip_prefix(UNNAMED)?;
    let (o, a) = match rest.split_once(".a") { Some((o, a)) => (o, Some(a.parse().ok()?)), None => (rest, None) };
    Some((ObjId(o.parse().ok()?), a))
}

/// Name every object in what is told to a seat by the seat's labels, giving a name to each
/// it has none for yet, in the order said.
fn name_objects(v: &mut Value, names: &mut game::prompt::Labels) {
    let name = |text: &str, names: &mut game::prompt::Labels| unnamed(text).map(|(o, a)| match a {
        Some(a) => Label::Ability(names.name(o), a).to_string(),
        None => Label::Object(names.name(o)).to_string(),
    });
    match v {
        Value::String(text) => if let Some(named) = name(text, names) { *text = named },
        Value::Array(items) => for item in items { name_objects(item, names) },
        Value::Object(map) => {
            for (k, item) in std::mem::take(map) {
                let mut item = item;
                name_objects(&mut item, names);
                map.insert(name(&k, names).unwrap_or(k), item);
            }
        }
        _ => {}
    }
}

/// What a seat's answer names by its labels, in the form the readers take ([`key`], [`aid`]):
/// a label it was never given names nothing.
fn unname_objects(v: &mut Value, names: &game::prompt::Labels) -> Result<(), Error> {
    match v {
        Value::String(text) => match Label::parse(text) {
            Some(Label::Object(n)) => *text = key_of(names, n, text)?,
            Some(Label::Ability(n, a)) => *text = format!("{}.a{a}", key_of(names, n, text)?),
            _ => {}
        },
        Value::Array(items) => for item in items { unname_objects(item, names)? },
        Value::Object(map) => for item in map.values_mut() { unname_objects(item, names)? },
        _ => {}
    }
    Ok(())
}

fn key_of(names: &game::prompt::Labels, n: u64, text: &str) -> Result<String, Error> {
    names.object(n).map(|o| format!("{UNNAMED}{}", o.0)).ok_or_else(|| Error::unreferable(format!("{text} names nothing you can refer to now")))
}

/// A player as the pipe names one where a label may stand: its seat, or its label.
fn pid(p: usize) -> Value {
    if labels() { json!(Label::Player(p).to_string()) } else { json!(p) }
}

fn target(t: &Target) -> Value {
    match t {
        Target::Player(p) if labels() => pid(*p),
        Target::Object(o) if labels() => oid(*o),
        Target::Player(p) => json!({ "player": p }),
        Target::Object(o) => json!({ "object": o.0 }),
    }
}

fn slot(s: &Slot) -> Value {
    let mut value = json!({ "among": s.among.iter().map(target).collect::<Vec<_>>(), "min": s.min, "max": s.max, "distinct_from": s.distinct_from, "divide": s.divide, "dependent": s.dependent.as_ref().map(|d|json!({"group":d.group,"options":d.options.iter().map(|(key,among)|json!({"target":target(key),"among":among.iter().map(target).collect::<Vec<_>>() })).collect::<Vec<_>>() })) });
    if let Some(pairs) = &s.controller_pairs {
        value["controller_pairs"] = json!(pairs.iter().map(|pair| pair.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>());
    }
    if let Some(partitions) = &s.same_graveyard {
        value["same_graveyard"] = json!(partitions.iter().map(|group| group.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>());
    }
    value
}

fn ids(v: &[ObjId]) -> Vec<Value> {
    v.iter().map(|o| oid(*o)).collect()
}

fn read_library_order(value: &Value) -> Result<Vec<ObjId>, String> {
    value.as_array().ok_or("order must be a top-first list of object IDs")?.iter()
        .map(|id| id.as_u64().map(ObjId).ok_or_else(|| "order needs unsigned object IDs".into())).collect()
}

fn read_shuffles(value: &Value) -> Result<Vec<game::start::ShuffleOrder>, String> {
    value.as_array().ok_or("shuffles must be a list of outcomes")?.iter().map(|item| {
        Ok(game::start::ShuffleOrder {
            player: item["player"].as_u64().ok_or("shuffle needs a player")? as usize,
            order: read_library_order(&item["order"])?,
        })
    }).collect()
}

fn read_random_outcomes(value: &Value) -> Result<Vec<game::start::RandomOutcome>, String> {
    use game::start::{RandomKind, RandomOutcome};
    let read = || -> Result<Vec<RandomOutcome>, String> {
        value.as_array().ok_or("random_outcomes must be a list")?.iter().map(|item| {
            let player = item.get("player").map(|value| value.as_u64().and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| "random outcome player must be unsigned".to_string())).transpose()?;
            if item["kind"] == "coin" {
                let object = item.as_object().expect("a coin outcome is an object");
                if object.keys().any(|key| !matches!(key.as_str(), "kind" | "player" | "result")) {
                    return Err("a coin receipt contains only kind, player and result".into());
                }
                let face = match item["result"].as_str() {
                    Some("heads") => game::start::CoinFace::Heads,
                    Some("tails") => game::start::CoinFace::Tails,
                    _ => return Err("a coin result is heads or tails".into()),
                };
                return Ok(RandomOutcome::Coin {player: player.ok_or("coin outcome needs player")?, face});
            }
            let kind = match item["kind"].as_str() {
                Some("shuffle") => RandomKind::Shuffle,
                Some("random_order") => RandomKind::RandomOrder,
                _ => return Err("unsupported random outcome kind".into()),
            };
            let order = read_library_order(&item["order"])?;
            Ok(match kind {
                RandomKind::Shuffle => RandomOutcome::Shuffle {player: player.ok_or("shuffle outcome needs player")?, order},
                RandomKind::RandomOrder => RandomOutcome::RandomOrder {player, order},
            })
        }).collect()
    };
    read().map_err(|why| game::start::ReplayInput(why).to_string())
}

fn random_outcome(kind: game::start::RandomKind, player: Option<usize>, position: Option<ir::action::LibraryPosition>, field: &str, order: Value) -> Value {
    use game::start::RandomKind;
    let mut result = json!({});
    if let Some(player) = player { result["player"] = json!(player); }
    result[field] = order;
    match kind {
        RandomKind::Shuffle => result["kind"] = json!("shuffle"),
        RandomKind::RandomOrder => {
            result["kind"] = json!("random_order");
            if let Some(position) = position { result["position"] = match position {
                ir::action::LibraryPosition::Top => json!("top"),
                ir::action::LibraryPosition::Bottom => json!("bottom"),
                ir::action::LibraryPosition::FromTop(n) => json!({"from_top":n}),
            }; }
        }
    }
    result
}

/// Mana as the letters of its symbols: `["R","G"]`.
fn mana(symbols: &[ManaSymbol]) -> Value {
    symbols.iter().map(|s| s.text()).collect()
}

fn manas(options: &[Vec<ManaSymbol>]) -> Value {
    options.iter().map(|o| mana(o)).collect()
}

fn read_mana(v: &Value) -> Result<Vec<ManaSymbol>, String> {
    let letters = v.as_array().ok_or("mana is a list of symbols, [\"R\",\"G\"]")?;
    letters.iter().map(|l| l.as_str().and_then(ManaSymbol::parse).ok_or_else(|| format!("{l} is no mana symbol"))).collect()
}

fn cost_selection((among, selection): &(Vec<ObjId>, game::legal::Selection)) -> Value {
    match selection {
        game::legal::Selection::Exact(count) => json!({"among":ids(among),"count":count}),
        game::legal::Selection::UpTo(maximum) => json!({"among":ids(among),"min":0,"max":(*maximum).min(among.len())}),
        game::legal::Selection::CardTypesAtLeast { minimum, types } => json!({"among":ids(among),"min":0,"max":among.len(),
            "card_types_at_least":minimum,"type_masks":types.iter().map(|(o,mask)|(key(*o),*mask)).collect::<std::collections::BTreeMap<_,_>>()}),
        // "with total power N or greater" (702.122a): each candidate's power now
        game::legal::Selection::PowerAtLeast { minimum, powers } => json!({"among":ids(among),"min":0,"max":among.len(),
            "power_at_least":minimum,"powers":powers.iter().map(|(o,p)|(key(*o),*p)).collect::<std::collections::BTreeMap<_,_>>()}),
        // collect evidence N (701.59a): each candidate card's mana value now
        game::legal::Selection::ManaValueAtLeast { minimum, values } => json!({"among":ids(among),"min":0,"max":among.len(),
            "mana_value_at_least":minimum,"mana_values":values.iter().map(|(o,v)|(key(*o),*v)).collect::<std::collections::BTreeMap<_,_>>()}),
        // waterbend (701.67a): up to one for each generic mana in it
        game::legal::Selection::Waterbend { generic, .. } => json!({"among":ids(among),"min":0,"max":(*generic as usize).min(among.len()),"waterbend":generic}),
    }
}

#[cfg(test)]
fn plain_selection(pool: &Pool) -> Value {
    assert!(pool.restricted.is_empty(), "the test renderer needs a real boundary to name restricted lots");
    let [w,u,b,r,g] = pool.colors;
    json!({"W":w,"U":u,"B":b,"R":r,"G":g,"C":pool.colorless})
}

#[cfg(test)]
fn payment(plan: &[Step]) -> Value {
    plan.iter()
        .map(|s| {
            let mut step = json!({ "source": oid(s.source), "ability": aid(s.source, s.ability), "paying": s.paying.iter().map(|g| ids(g)).collect::<Vec<_>>(), "adding": manas(&s.adding) });
            // `"chose":[1]`: the alternative of its cost paid, said only of a cost that has them (601.2b)
            if !s.chose.is_empty() { step["chose"] = json!(s.chose); }
            if !s.plan.is_empty() {step["plan"]=payment(&s.plan);}
            if let Some(x)=s.x {step["x"]=json!(x);}
            if !s.life.is_empty() {step["life"]=json!(s.life);}
            if let Some(branches)=&s.cost_branches {step["cost_branches"]=json!(branches);}
            if let Some(with) = &s.with { step["mana"] = plain_selection(with); }
            step
        })
        .collect()
}





fn zone_name(z: Zone) -> &'static str { match z { Zone::Library => "library", Zone::Hand => "hand", Zone::Battlefield => "battlefield", Zone::Graveyard => "graveyard", Zone::Stack => "stack", Zone::Exile => "exile", Zone::Command => "command", Zone::Gone => "gone" } }

/// Where chosen objects go: `{"zone":"hand"}`, `{"zone":"library","position":"top"}`, or
/// `{"zone":"library","from_top":2}` (400.3, 401.4).
fn placement(to: game::state::Placement) -> Value {
    use ir::action::LibraryPosition;
    match to {
        game::state::Placement::Zone(z) => json!({ "zone": zone_name(z) }),
        game::state::Placement::Library(LibraryPosition::Top) => json!({ "zone": "library", "position": "top" }),
        game::state::Placement::Library(LibraryPosition::Bottom) => json!({ "zone": "library", "position": "bottom" }),
        game::state::Placement::Library(LibraryPosition::FromTop(n)) => json!({ "zone": "library", "from_top": n }),
    }
}

fn disclosure(d: &Disclosure) -> Value {
    json!({
        "seq": d.seq,
        "kind": match d.kind { Kind::Looked => "looked", Kind::Revealed => "revealed" },
        "by": d.by,
        "cards": d.cards.iter().map(|c| json!({ "object": oid(c.object), "name": c.name, "owner": c.owner, "zone": zone_name(c.zone) })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
fn asked(g: &mut Game) -> Value {
    let question = g.asked();
    render_asked(g, question)
}

/// `moves`: what the player with priority could do, when it was asked for.
fn render_asked(g: &Game, question: Option<(usize, Asked)>) -> Value {
    let Some((acting, what)) = question else { return Value::Null };
    let player=g.decision_maker(acting);
    let mut v = match what {
        Asked::Payment { source, reason } => json!({"kind":"payment", "source":oid(source), "reason":reason, "can_reverse":true}),
        Asked::Priority => json!({ "kind": "priority" }),
        Asked::ChangeTargets { object, original, slots, policy } => json!({ "kind": "change_targets", "object": oid(object), "original": original.iter().map(|g| g.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>(), "slots": slots.iter().map(slot).collect::<Vec<_>>(), "policy": match policy { game::state::TargetPolicy::RequiredSingle => "required_single", game::state::TargetPolicy::ChooseNew => "choose_new" } }),
        Asked::Retarget { copy, original, slots } => json!({ "kind": "retarget", "copy": oid(copy), "original": original.iter().map(|g| g.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>(), "slots": slots.iter().map(slot).collect::<Vec<_>>() }),
        Asked::Targets { source, slots } => {
            let mut question = json!({ "kind": "targets", "source": source.map(|source| g.obj(source).chars.name.clone()), "slots": slots.iter().map(slot).collect::<Vec<_>>() });
            if let Some(game::state::Decision::Targets { modes, .. }) = &g.decision {
                if !modes.is_empty() { question["modes"] = json!(modes); }
            }
            question
        },
        // the choices of modes, each with what its targets could be (603.3c, 700.2c)
        Asked::Modes { source, options } => json!({
            "kind": "modes",
            "source": source.map(|source| g.obj(source).chars.name.clone()),
            "options": options.iter().map(|(modes, slots)| json!({ "modes": modes, "slots": slots.iter().map(slot).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        }),
        Asked::Discard { among, count } => json!({ "kind": "discard", "among": ids(&among), "count": count }),
        Asked::Mulligan => json!({ "kind": "mulligan" }),
        Asked::Bottom { among, count } => json!({ "kind": "bottom", "among": ids(&among), "count": count }),
        Asked::Attackers(able) => json!({
            "kind": "attackers",
            "able": able
                .iter()
                .map(|a| json!({ "creature": oid(a.creature), "attacks": a.attacks.iter().map(target).collect::<Vec<_>>(), "must": a.must, "costs": a.costs.iter().map(|(t, c)| json!({"target": target(t), "cost": mana(&c.0)})).collect::<Vec<_>>() }))
                .collect::<Vec<_>>(),
        }),
        Asked::Blockers { able, together, capacity } => json!({
            "kind": "blockers",
            "able": able.iter().map(|(o, what)| json!({ "creature": oid(*o), "blocks": ids(what), "capacity": capacity.iter().find(|(b, _)| b == o).map_or(1, |(_, n)| *n) })).collect::<Vec<_>>(),
            "together": together.iter().map(|(attacker, count)| json!({ "attacker": oid(*attacker), "count": count })).collect::<Vec<_>>(),
        }),
        Asked::Divide { attacker, among, amount, lethal } => {
            json!({ "kind": "divide", "attacker": oid(attacker), "among": among.iter().map(target).collect::<Vec<_>>(), "amount": amount, "lethal": lethal })
        }
        Asked::Alternative { among, optional } => json!({ "kind": "alternative", "optional": optional, "among": among.iter().map(|option| match &option.body {
            game::state::ActionOptionBody::Objects { verb, among, count } => json!({"index":option.index,"action":format!("{verb:?}"),"among":ids(among),"count":count}),
            game::state::ActionOptionBody::Counters { on, kind, count } => json!({"index":option.index,"action":"PutCounters","on":oid(*on),"counter":kind,"count":count}),
            game::state::ActionOptionBody::Create { tokens } => json!({"index":option.index,"action":"Create","tokens":tokens.iter().map(|(kind,count)|json!({"name":kind.name(),"count":count})).collect::<Vec<_>>()}),
        }).collect::<Vec<_>>() }),
        // what it lets the player do, when the instruction says (608.2d)
        Asked::May => match g.optional() {
            Some((verb, to)) => json!({ "kind": "may", "optional": { "verb": format!("{verb:?}"), "to": to.map(placement) } }),
            None => json!({ "kind": "may" }),
        },
        // answered with one of the moves, or "no" (608.2g)
        Asked::Cast { cards } => json!({ "kind": "cast", "cards": ids(&cards) }),
        Asked::Pay { cost, life, waterbend } => {
            let mut v = json!({ "kind": "pay", "cost": mana(&cost.0) });
            // the life the payment costs besides its mana (119.4)
            if life > 0 { v["life"] = json!(life); }
            // of a waterbend cost, the artifacts and creatures its payer may tap for it, each
            // paying {1} of its generic mana, as many as there are of either (701.67a): answered
            // with `"paying": [[those tapped]]`; `tapped`, those the found plan taps, which a
            // plain yes follows. `waterbend` is the cost's generic mana, as listings say it.
            if let Some(bend) = waterbend {
                v["paying"] = json!([{ "among": ids(&bend.among), "min": 0, "max": bend.most, "waterbend": bend.generic }]);
            }
            v
        }
        Asked::CommandZone { object } => json!({ "kind": "command_zone", "object": oid(object) }),
        Asked::Holders { verb, among } => json!({ "kind": "holders", "verb": format!("{verb:?}"), "among": among.iter().map(target).collect::<Vec<_>>() }),
        Asked::Select { among, min, max } => {
            let mut v = json!({ "kind": "select", "among": ids(&among), "min": min, "max": max });
            // the cards selected are one pile, the rest the other (700.3)
            if g.separating() { v["separate"] = json!(true); }
            v
        }
        Asked::Search { cards, among, min, max } => json!({ "kind": "search", "cards": ids(&cards), "among": ids(&among), "min": min, "max": max }),
        Asked::Attach { aura, among } => json!({ "kind": "attach", "aura": oid(aura), "among": among.iter().map(target).collect::<Vec<_>>() }),
        // what a creature put onto the battlefield attacking attacks (508.4)
        Asked::Defender { among, nth, of } => json!({ "kind": "defender", "among": among.iter().map(target).collect::<Vec<_>>(), "nth": nth, "of": of }),
        Asked::ConstrainedObjects { verb, among, selection } => {
            let mut v = cost_selection(&(among, selection));
            v["kind"] = json!("objects"); v["verb"] = json!(format!("{verb:?}")); v
        },
        Asked::Objects { verb, to, among, count } => json!({ "kind": "objects", "verb": format!("{verb:?}"), "to": to.map(placement), "among": ids(&among), "count": count }),
        Asked::OrderCards { to, cards } => {
            let mut asked=json!({ "kind": "order_cards", "to": placement(to), "cards": ids(&cards) });
            if let Some(object)=g.library_arrangement() { asked["arrange"]=json!({"object":object.0}); }
            asked
        },
        Asked::OptionalObjects { verb, to, among, count } => json!({ "kind": "optional_objects", "verb": format!("{verb:?}"), "to": to.map(placement), "among": ids(&among), "count": count }),
        Asked::Mana { options } => {
            let mut asked=json!({ "kind": "mana", "options": manas(&options) });
            if let Some(at)=g.resolving() {asked["source"]=json!(at.object.0);}
            asked
        },
        Asked::LibraryDestination { cards, positions } => json!({ "kind": "library_destination", "cards": ids(&cards),
            "positions": positions.into_iter().map(|p| placement(game::state::Placement::Library(p))).collect::<Vec<_>>() }),
        Asked::Number { min, max } => json!({ "kind": "number", "min": min, "max": max }),
        Asked::CoinCall => json!({"kind":"coin_call","options":["heads","tails"]}),
        // what is chosen: a color (105.1), a creature type (205.3m), a card name (201.4), a
        // card type (205.2a) or a basic land type (305.6)
        Asked::Value { kind, options } => json!({ "kind": "value", "of": value_kind(kind), "options": options.iter().map(bound_value).collect::<Vec<_>>() }),
        // the cards looked at, the first of them the top card (701.22a, 701.25a)
        Asked::Arrange { verb, cards } => json!({ "kind": "arrange", "verb": format!("{verb:?}"), "cards": ids(&cards) }),
        Asked::Player { among } => json!({ "kind": "player", "among": among }),
        // answered with the place of one of them (700.3)
        Asked::Pile { piles } => json!({ "kind": "pile", "piles": piles.iter().map(|pile| ids(pile)).collect::<Vec<_>>() }),
        Asked::SpellDestination{on,among}=>json!({"kind":"replacement","event":"spell_destination","on":target(&Target::Object(on)),"optional":false,
            // each effect named, and whose it is (616.1)
            "among":among.iter().map(|e| {
                let mut v = match g.spell_destination_source(on, *e) {
                    game::completion::DestinationSource::Ability { source, ability } => json!({ "source": source.map(oid), "ability": ability.map(replacement_ability) }),
                    game::completion::DestinationSource::Replacement(id) => replacement(&id),
                    game::completion::DestinationSource::Rule(rule) => json!({ "rule": rule }),
                };
                v["effect"] = json!(e.name());
                v
            }).collect::<Vec<_>>() }),
        Asked::DamageOrder{on,hits,among}=>json!({
            "kind":"replacement","event":"damage_prevention","on":target(&on),"optional":false,
            "hits":hits.iter().map(|h|json!({"hit":h.id,"source":oid(h.source),"amount":h.amount})).collect::<Vec<_>>(),
            "among":among.iter().map(|o|damage_option_display(g,o)).collect::<Vec<_>>() }),
        Asked::Prevention{on,shield,total,hits}=>json!({
            "kind":"prevention","on":target(&on),"shield":replacement(&shield),"total":total,
            "hits":hits.iter().map(|h|json!({"hit":h.id,"source":oid(h.source),"amount":h.amount})).collect::<Vec<_>>() }),
        Asked::DamageReplacement {on,hits,among} => json!({
            "kind":"replacement", "event":"damage", "on":target(&on), "optional":false,
            "hits":hits.iter().map(|h|json!({"hit":h.id,"source":oid(h.source),"amount":h.amount})).collect::<Vec<_>>(),
            "among":among.iter().map(|o| {let mut v=damage_offer(o);v["damage_source"]=json!(oid(o.source));v["name"]=json!(g.obj(o.effect.source).chars.name);v}).collect::<Vec<_>>()
        }),
        // which skip effect replaces the step or phase that would begin (614.10a, 616.1)
        Asked::Skip { what, among } => json!({
            "kind": "skip", "what": occurrence(what),
            "among": among.iter().map(skip_effect).collect::<Vec<_>>(),
        }),
        Asked::Replacement { on, kind, count, counters, tokens, among, optional } => {
            let mut shown=json!({
            "kind": "replacement", "on": target(&on), "optional": optional,
            "event": if matches!(kind.as_str(), "draw" | "death" | "graveyard" | "create" | "discard") { kind.as_str() } else if g.entering_object().map(Target::Object) == Some(on) { "entry" } else { "put_counters" },
            "counter": match counters.as_ref().map(|c|c.inventory.as_slice()) {
                Some([(kind,_)])=>json!(kind), Some(_)=>Value::Null,
                None=>Value::Null,
            }, "count": counters.as_ref().map_or(Some(count), |c|c.inventory.first().filter(|_|c.inventory.len()==1).map(|(_,n)|*n)),
            "among": among.iter().map(|e| { let mut value=replacement(e); value["name"]=json!(g.obj(e.source).chars.name);
                if tokens.is_some() { let mut producer=g.audit_creation_replacement(e); producer["source"]=oid(e.source); value["producer"]=producer; }
                value }).collect::<Vec<_>>(),
            });
            if let Some(tokens)=tokens {
                shown["count"]=Value::Null;
                shown["by"]=json!(tokens.by);
                shown["instructions"]=tokens.instructions.as_ref().map_or(Value::Null,|i|json!({"tapped":i.tapped,"choose_defender":i.choose_defender,"attacking":i.attacking.as_ref().map(target),"attached_to":i.attached.as_ref().map(target)}));
                shown["tokens"]=json!(tokens.inventory.iter().map(|token|json!({
                    "specification":token.specification,"name":token.chars.name,"count":token.count,"types":token.chars.types,
                    "colors":token.chars.colors.iter().map(|c|c.word()).collect::<Vec<_>>(),"power":token.chars.pt.map(|pt|pt.0),"toughness":token.chars.pt.map(|pt|pt.1),
                    "tapped":token.tapped,"choose_defender":token.choose_defender,"attacking":token.attacking.as_ref().map(target),"attached_to":token.attached.as_ref().map(target),
                })).collect::<Vec<_>>());
            }
            if let Some(counters)=counters {
                shown["counters"]=json!(counters.inventory.iter().map(|(kind,count)|json!({"counter":kind,"count":count})).collect::<Vec<_>>());
                shown["by"]=json!(counters.by);
            }
            shown
        },
        Asked::Order(abilities) => json!({
            "kind": "order",
            "abilities": abilities.iter().map(|(source, key)| {
                use game::binding::Origin;
                let grant = match key.origin {
                    Origin::Given { grant, .. } => json!({"source": grant.by.map(oid), "stamp": grant.stamp, "part": grant.part}),
                    _ => Value::Null,
                };
                json!({ "source": source.map(oid), "ability": key.index, "origin": key.origin.clone().name(), "grant": grant,
                    "text": g.ability_text(*source, key.clone()) })
            }).collect::<Vec<_>>(),
        }),
    };
    v["player"] = json!(player);
    if acting != player { v["acting"] = json!(acting); }
    if let Some(card) = g.entering_card() { v["entering"] = oid(card); }
    v
}

/// An event as a player saw it (`viewer`; None: as it was): an object whose face the player
/// could not see is null (400.2, 402.3). Cards revealed are shown to all players, by name
/// (701.20a).
fn event(g: &Game, viewer: Option<usize>, e: &Event) -> Value {
    // a face-down permanent is there for every player, whoever may look at its face (708.5)
    let o = |id: &ObjId| if viewer.is_none_or(|v| g.perceives(v, *id)) { oid(*id) } else { Value::Null };
    let zone = |z: &Zone| format!("{z:?}");
    let (kind, mut v) = match e {
        Event::ManaAdded { source, player, mana: symbols } => ("mana_added", json!({ "source": o(source), "player": pid(*player), "mana": mana(symbols) })),
        Event::TappedForMana { object, player } => ("tapped_for_mana", json!({ "object": o(object), "player": pid(*player) })),
        Event::Shuffled { player, by } => ("shuffled", json!({ "player": pid(*player), "by": pid(*by) })),
        Event::CoinFlipped { source, result } => ("coin_flipped", json!({"source":o(source),"player":pid(result.player),
            "call":result.call.map(|face|face.name()),"result":result.face.name(),"won":result.won()})),
        Event::BecameTarget { object, of, .. } => ("became_target", json!({ "object": target(object), "of": o(of) })),
        Event::Countered { object } => ("countered", json!({ "object": o(object) })),
        Event::MonarchChanged { player, previous } => ("monarch",json!({"player":player.map(pid),"previous":previous.map(pid)})),
        Event::SpeedChanged { player, speed, previous } => ("speed", json!({ "player": pid(*player), "speed": speed, "previous": previous })),
        Event::Plotted { player, card } => ("plotted",json!({"player":pid(*player),"card":o(card)})),
        Event::Foretold { player, card } => ("foretold",json!({"player":pid(*player),"card":o(card)})),
        Event::Crews { vehicle, creatures } => ("crews",json!({"vehicle":o(vehicle),"creatures":creatures.iter().map(|c|o(c)).collect::<Vec<_>>()})),
        Event::Saddles { mount, creatures } => ("saddles",json!({"mount":o(mount),"creatures":creatures.iter().map(|c|o(c)).collect::<Vec<_>>()})),
        Event::BecameCrewed { vehicle, by } => ("became_crewed",json!({"vehicle":o(vehicle),"by":by.iter().map(|c|o(c)).collect::<Vec<_>>()})),
        Event::Drew { player, card, .. } => ("drew", json!({ "player": pid(*player), "card": o(card) })),
        Event::Attached { aura, to } => ("attached", json!({ "aura": o(aura), "to": target(to) })),
        Event::Milled { player, card } => ("milled", json!({ "player": pid(*player), "card": o(card) })),
        Event::Revealed { player, cards } => {
            // named as the disclosure of the same reveal names them: a face-down permanent
            // as the physical card it is (708.9, 701.40g)
            let names: Vec<&str> = cards.iter().map(|c| g.shown_name(*c)).collect();
            ("revealed", json!({ "player": pid(*player), "cards": ids(cards), "names": names }))
        }
        Event::Beheld { player, permanents } => ("beheld", json!({ "player": pid(*player), "permanents": ids(permanents) })),
        Event::Connived {object,player} => ("connived",json!({"object":oid(*object),"player":pid(*player)})),
        Event::CrimeCommitted { player } => ("crime_committed",json!({"player":pid(*player)})),
        Event::EvidenceCollected { player } => ("evidence_collected",json!({"player":pid(*player)})),
        Event::Discovered { player, value } => ("discovered",json!({"player":pid(*player),"value":value})),
        Event::Explored {object,player} => ("explored",json!({"object":oid(*object),"player":pid(*player)})),
        Event::Arranged { player, verb } => ("arranged", json!({ "player": pid(*player), "verb": format!("{verb:?}") })),
        Event::ExcessDamageDealt { to, amount, combat } => ("excess_damage", json!({ "to": target(to), "amount": amount, "combat": combat })),
        Event::DamageDealt { source, to, amount, combat } => ("damage", json!({ "source": o(source), "to": target(to), "amount": amount, "combat": combat })),
        Event::Destroyed { object } => ("destroyed", json!({ "object": o(object) })),
        Event::Regenerated { object } => ("regenerated", json!({ "object": o(object) })),
        Event::Exiled { object } => ("exiled", json!({ "object": o(object) })),
        Event::Moved { object, to } => ("moved", json!({ "object": o(object), "to": zone(to) })),
        Event::LifeGained { player, amount } => ("life_gained", json!({ "player": pid(*player), "amount": amount })),
        Event::LifeLost { player, amount } => ("life_lost", json!({ "player": pid(*player), "amount": amount })),
        Event::Tapped { object } => ("tapped", json!({ "object": o(object) })),
        Event::Untapped { object } => ("untapped", json!({ "object": o(object) })),
        Event::Transformed { object } => ("transformed", json!({ "object": o(object) })),
        Event::BecameLevel { object, level } => ("became_level", json!({ "object": o(object), "level": level })),
        Event::Designated { object, designation } => ("designated", json!({ "object": o(object), "designation": match designation {
            ir::condition::ObjectDesignation::Solved => "solved",
            ir::condition::ObjectDesignation::Monstrous => "monstrous",
            ir::condition::ObjectDesignation::Renowned => "renowned",
            ir::condition::ObjectDesignation::Saddled => "saddled",
            ir::condition::ObjectDesignation::Suspended => "suspended",
            ir::condition::ObjectDesignation::Prepared => "prepared",
        } })),
        Event::Unlocked { object, half, player } => ("unlocked", json!({ "object": o(object), "half": half, "player": player })),
        Event::TurnedFaceUp { object } => ("turned_face_up", json!({ "object": o(object) })),
        Event::TurnedFaceDown { object } => ("turned_face_down", json!({ "object": o(object) })),
        Event::CountersPut { on, kind, count, by, .. } => ("counters", json!({ "on": target(on), "counter": kind, "count": count, "by": pid(*by) })),
        Event::CountersRemoved { from, kind, count } => ("counters_removed", json!({ "from": target(from), "counter": kind, "count": count })),
        Event::EmblemCreated { object } => ("emblem_created", json!({ "object": o(object) })),
        Event::Created { token } => ("created", json!({ "token": o(token) })),
        Event::AbilityCopied { original, copy, by } => ("ability_copied", json!({ "original": o(original), "copy": o(copy), "player": pid(*by) })),
        Event::SpellCopied { original, copy, by } => ("spell_copied", json!({ "original": o(original), "copy": o(copy), "player": pid(*by) })),
        Event::Cast { spell, by } => ("cast", json!({ "spell": o(spell), "player": pid(*by) })),
        Event::LandPlayed { land, by } => ("land_played", json!({ "land": o(land), "player": pid(*by) })),
        Event::Activated { source, by, ability, keyword } => ("activated", json!({ "source": o(source), "player": pid(*by), "ability": ability.as_ref().map(o), "keyword": keyword.map(|k| k.name()) })),
        Event::ZoneChanged { was, now, from, to } => ("zone", json!({ "was": o(was), "now": now.as_ref().map(o), "from": zone(from), "to": zone(to) })),
        Event::Resolved { spell } => ("resolved", json!({ "spell": o(spell) })),
        Event::AnnouncementReversed { source, restored, player } => ("announcement_reversed", json!({"source":o(source), "restored":restored.as_ref().map(o), "player":pid(*player)})),
        Event::Fizzled { spell } => ("fizzled", json!({ "spell": o(spell) })),
        Event::Discarded { player, card, .. } => ("discarded", json!({ "player": pid(*player), "card": o(card) })),
        Event::Sacrificed { player, object } => ("sacrificed", json!({ "player": pid(*player), "object": o(object) })),
        Event::Attacked { attacker, target: t } => ("attacked", json!({ "attacker": o(attacker), "target": target(t) })),
        Event::AttackersDeclared { player } => ("attackers_declared", json!({ "player": pid(*player) })),
        Event::BecameBlocking { blocker } => ("became_blocking", json!({ "blocker": o(blocker) })),
        Event::Blocked { blocker, attacker } => ("blocked", json!({ "blocker": o(blocker), "attacker": o(attacker) })),
        Event::BecameBlocked { attacker } => ("became_blocked", json!({ "attacker": o(attacker) })),
        Event::TurnBegan { player } => ("turn_began", json!({ "player": pid(*player) })),
        Event::Began { part } => ("began", json!({ "part": format!("{part:?}") })),
        Event::Skipped { player, what, skip } => ("skipped", json!({ "player": pid(*player), "what": occurrence(*what), "skip": skip.0 })),
        Event::Conceded { player } => ("conceded", json!({ "player": pid(*player) })),
        Event::Lost { player } => ("lost", json!({ "player": pid(*player) })),
    };
    v["event"] = json!(kind);
    v
}

/// The game: the turn, each player, the public zones, and the objects in them and in the
/// hands. `viewer`: the player it is shown to, who sees what that player may (400.2): of the
/// hidden zones their own hand, of another's hand and of every library how many cards (402.3,
/// 401.3); what is asked only if it is asked of them; what happened since they last looked,
/// as they saw it. None: all of it, as the harness that sets a game up and compares it sees.
#[cfg(test)]
fn state(g: &mut Game, viewer: Option<usize>, logged: &mut Delivery) -> Value {
    let asking = asked(g);
    logged.snapshot(g);
    render_state(g, asking, viewer, logged)
}

/// What happened, as each viewer was allowed to see it when it happened, waiting for that
/// viewer to ask: the host's own record, outside the game and its copies. Events are
/// rendered once, as soon as the game has them ([`Delivery::snapshot`], after every
/// command), so nothing here reads the game's history again and the game may drop events
/// it has no more use for. A viewer's events leave when that viewer is answered; those of
/// a viewer that never asks wait, which is this host's buffering policy for one pipe.
#[derive(Clone, Debug, Default, PartialEq)]
struct Delivery {
    /// How much of the game's log has been rendered: the one place a log position is kept.
    rendered: game::event::Seq,
    /// For each seat, and None for the harness that sees the whole game.
    waiting: BTreeMap<Option<usize>, Vec<Value>>,
    /// The events as the engine prints them, for the harness.
    debug: Vec<String>,
    /// For each seat that has been prompted (`prompt`), the events since its last prompt, said
    /// with labels as that seat saw them. Kept apart from `waiting`: a prompt and a state are
    /// two ways to look, each told once.
    prompted: BTreeMap<usize, Vec<Value>>,
}

impl Delivery {
    /// Render what the game has logged since the last time, for every viewer.
    fn snapshot(&mut self, g: &Game) {
        for e in g.log.since(self.rendered) {
            for viewer in std::iter::once(None).chain((0..g.players.len()).map(Some)) {
                self.waiting.entry(viewer).or_default().push(event(g, viewer, e));
            }
            for (seat, waiting) in &mut self.prompted {
                waiting.push(labelled(|| event(g, Some(*seat), e)));
            }
            self.debug.push(format!("{e:?}"));
        }
        self.rendered = g.log.end();
    }

    fn take(&mut self, viewer: Option<usize>) -> Vec<Value> {
        self.waiting.get_mut(&viewer).map(std::mem::take).unwrap_or_default()
    }

    /// The events since this seat was last prompted; from its first prompt on.
    fn take_prompted(&mut self, seat: usize) -> Vec<Value> {
        std::mem::take(self.prompted.entry(seat).or_default())
    }
}

/// A replacement effect as an answer names it.
fn damage_option_display(g:&Game,offer:&game::operation::DamageOption)->Value {
    use game::operation::DamageOption;
    let mut v=damage_option(offer);
    match offer {
        DamageOption::Modifier(o)=>{v["name"]=json!(g.obj(o.effect.source).chars.name);v["damage_source"]=json!(oid(o.source));},
        DamageOption::Protection{source,..}=>{v["name"]=json!("Protection");v["damage_source"]=json!(oid(*source));},
        DamageOption::Prevention{effect,source,..}=>{v["name"]=json!(g.obj(effect.source).chars.name);v["damage_source"]=json!(oid(*source));},
        DamageOption::Shield(id)=>{v["name"]=json!(g.obj(id.source).chars.name);v["remaining"]=json!(g.prevention_capacity(id.clone()));},
    }
    v
}
fn damage_option(offer:&game::operation::DamageOption)->Value {
    use game::operation::DamageOption;
    match offer {
        DamageOption::Modifier(o)=>{let mut v=damage_offer(o);v["kind"]=json!("modifier");v},
        DamageOption::Protection{hit,..}=>json!({"kind":"protection","hit":hit}),
        DamageOption::Prevention{effect,hit,..}=>{let mut v=replacement(effect);v["kind"]=json!("prevent_all");v["hit"]=json!(hit);v},
        DamageOption::Shield(id)=>{let mut v=replacement(id);v["kind"]=json!("shield");v},
    }
}
fn read_damage_order(v:&Value,among:&[game::operation::DamageOption])->Result<Move,Error> {
    if v["kind"]!="choose" {return Err("choose an offered damage/prevention identity".into());}
    let answer=&v["answer"]["damage_order"];
    let offer=among.iter().find(|o| {
        let identity=damage_option(o);
        identity.as_object().unwrap().iter().all(|(k,value)|answer.get(k)==Some(value))
            && answer.as_object().is_some_and(|a|a.keys().all(|k|identity.get(k).is_some() || matches!(k.as_str(),"name"|"remaining"|"damage_source")))
    }).ok_or_else(|| Error::unreferable("that damage/prevention identity was not offered"))?;
    Ok(Move::Choose(Answer::DamageOrder(offer.clone())))
}
fn damage_offer(offer: &game::operation::DamageOffer) -> Value {
    let mut v=replacement(&offer.effect);
    v["hit"]=json!(offer.hit);
    v
}
fn read_damage_move(v:&Value,among:&[game::operation::DamageOffer])->Result<Move,Error> {
    if v["kind"]!="choose" { return Err("choose an offered damage replacement and hit (616.1)".into()); }
    let answer=&v["answer"]["replacement"];
    let identity=json!({"source":answer["source"],"ability":answer["ability"],"part":answer["part"],"hit":answer["hit"]});
    let offer=among.iter().find(|o|damage_offer(o)==identity)
        .ok_or_else(|| Error::unreferable("that damage replacement/hit identity was not offered"))?;
    Ok(Move::Choose(Answer::DamageReplacement(offer.clone())))
}
fn replacement(effect: &game::operation::ReplacementId) -> Value {
    match &effect.effect {
        game::operation::ReplacementEffect::Ability { key, part } => json!({ "source": oid(effect.source), "ability": replacement_ability(key.clone()), "part": part }),
        game::operation::ReplacementEffect::Finality => json!({ "source": oid(effect.source), "rule": "finality" }),
    }
}

/// An answer the engine chose, as [`read_move`] reads it: said back to the engine as the
/// player's move, it is the same answer.
/// What a value question chooses, as its answer's key says it (`{"value":{"color":"red"}}`).
fn value_kind(kind: game::binding::Kind) -> &'static str {
    use game::binding::Kind;
    match kind {
        Kind::Color => "color",
        Kind::CreatureType => "creature_type",
        Kind::CardName => "card_name",
        Kind::CardType => "card_type",
        Kind::BasicLandType => "basic_land_type",
        Kind::Type => "type",
        Kind::Cards => "cards",
        Kind::Destructions => "destructions",
        Kind::Removals => "removals",
        Kind::Damage => "damage",
        Kind::Subjects => "subjects",
        Kind::Consent => "consent",
        Kind::Number => "number",
        Kind::Coin => "coin",
        Kind::Turn => "turn",
        Kind::CreatedTurns => "created_turns",
    }
}

/// A step or phase as a skip effect names it: `{"step": "Draw"}`, `{"phase": "Combat"}`.
fn occurrence(what: game::turn::Occurrence) -> Value {
    match what {
        game::turn::Occurrence::Turn => json!({ "turn": true }),
        game::turn::Occurrence::Part(part) => json!({ "step": format!("{part:?}") }),
        game::turn::Occurrence::Phase(phase) => json!({ "phase": format!("{phase:?}") }),
    }
}

/// A skip effect: answered by `{"skip": id}`.
fn skip_effect(skip: &game::turn::Skip) -> Value {
    let lasts = match skip.lasts {
        game::turn::SkipLasts::Next => json!("next"),
        game::turn::SkipLasts::NextThisTurn { turn } => json!({ "this_turn": turn }),
    };
    // the object and ability whose effect it is (614.10, 616.1)
    json!({ "id": skip.id.0, "player": skip.player, "what": occurrence(skip.what), "lasts": lasts,
        "source": skip.source.map(oid), "ability": skip.ability.clone().map(replacement_ability) })
}

#[cfg(test)]
fn answered(mv: &Move) -> Value {
    match mv {
        Move::Pass => json!({ "kind": "pass" }),
        Move::Concede => json!({ "kind": "concede" }),
        Move::Order(order) => json!({ "kind": "order", "order": order }),
        Move::ChooseModes(modes) => json!({ "kind": "modes", "modes": modes }),
        Move::Discard(cards) => json!({ "kind": "discard", "cards": ids(cards) }),
        Move::Keep => json!({ "kind": "keep" }),
        Move::Bottom(cards) => json!({ "kind": "bottom", "cards": ids(cards) }),
        Move::Attack(attacks) => json!({ "kind": "attack", "attacks": attacks.iter().map(|(a, t)| json!([oid(*a), target(t)])).collect::<Vec<_>>() }),
        Move::Block(blocks) => json!({ "kind": "block", "blocks": blocks.iter().map(|(b, a)| json!([oid(*b), oid(*a)])).collect::<Vec<_>>() }),
        Move::Choose(answer) => json!({ "kind": "choose", "answer": match answer {
            Answer::CoinCall(face) => json!({"call":face.name()}),
            Answer::SpellDestination{on,effect}=>json!({"spell_destination":{"on":oid(*on),"effect":effect.name()}}),
            Answer::Targets(groups) => json!({ "targets": groups.iter().map(|g|g.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>() }),
            Answer::Yes => json!("yes"),
            Answer::No => json!("no"),
            Answer::Pay { plan, with, tapped } => {
                let mut pay = json!({ "plan": payment(plan) });
                if let Some(with) = with { pay["mana"] = plain_selection(with); }
                if !tapped.is_empty() { pay["paying"] = json!([ids(tapped)]); }
                json!({ "pay": pay })
            }
            Answer::Objects(objects) => json!({ "objects": ids(objects) }),
            Answer::OptionalObjects(Some(objects)) => json!({ "accept": true, "objects": ids(objects) }),
            Answer::OptionalObjects(None) => json!({ "accept": false }),
            Answer::Holders(holders) => json!({ "holders": holders.iter().map(target).collect::<Vec<_>>() }),
            Answer::Attach(to) => json!({ "attach": target(to) }),
            Answer::Defender(to) => json!({ "defender": target(to) }),
            Answer::Mana(symbols) => json!({ "mana": mana(symbols) }),
            Answer::Value(value) => json!({ "value": bound_value(value) }),
            Answer::Arrange { top, away } => json!({ "top": ids(top), "away": ids(away) }),
            Answer::Replacement(effect) => json!({ "replacement": replacement(effect) }),
            Answer::DamageOrder(offer)=>json!({"damage_order":damage_option(offer)}),
            Answer::Prevention(allocation)=>json!({"prevention":allocation}),
            Answer::DamageReplacement(offer) => json!({"replacement":damage_offer(offer)}),
            Answer::Player(player) => json!({ "player": player }),
            Answer::Alternative(index) => json!({ "alternative": index }),
            Answer::LibraryPosition(p) => json!({"library_position": placement(game::state::Placement::Library(*p))}),
            Answer::Pile(pile) => json!({ "pile": pile }),
            Answer::Skip(id) => json!({ "skip": id.0 }),
        } }),
        // This serializer covers recorded choice answers, not arbitrary action commands.
        other => json!({ "kind": "other", "debug": format!("{other:?}") }),
    }
}

#[cfg(test)]
fn render_state(g: &Game, asking: Value, viewer: Option<usize>, logged: &mut Delivery) -> Value {
    render_snapshot(&Snapshot::of(g, viewer), asking, logged)
}

/// The state as the pipe says it: the game as its viewer may see it ([`Snapshot`], which
/// the simulation keeps while the game is unchanged), with what is delivered once, what is
/// asked and what happened, put to it here each time.
fn render_snapshot(seen: &Snapshot, mut asking: Value, logged: &mut Delivery) -> Value {
    let viewer = seen.viewer;
    let events = logged.take(viewer);
    // what is asked says what the asked player could do, and with what: theirs to see
    if let (Some(v), Some(player)) = (viewer, asking.get("player").and_then(Value::as_u64)) {
        if player as usize != v && asking.get("acting").and_then(Value::as_u64) != Some(v as u64) {
            let acting=asking.get("acting").cloned();
            asking = json!({ "kind": asking["kind"], "player": player });
            if let Some(acting)=acting { asking["acting"]=acting; }
        }
    }
    let mut state = render_view(seen);
    state["asked"] = asking;
    state["events"] = events.into();
    if viewer.is_none() {
        state["log"] = std::mem::take(&mut logged.debug).into();
    }
    state
}

/// The game as its viewer may see it, without what is asked or delivered once: what the
/// pipe's state and a player's prompt both say of it.
fn render_view(seen: &Snapshot) -> Value {
    let viewer = seen.viewer;
    let mut objects = BTreeMap::new();
    for o in &seen.objects {
        let mut v = json!({
            "name": o.name,
            "owner": pid(o.owner),
            "controller": pid(o.controller),
            "types": o.types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>(),
            "subtypes": o.subtypes.iter().map(|s| s.name()).collect::<Vec<_>>(),
            "supertypes": o.supertypes.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>(),
            "colors": o.colors.iter().map(|c| c.word()).collect::<Vec<_>>(),
            "token": o.token,
            "is_card": o.is_card,
        });
        // its power and toughness, if it has them: a permanent's only while it is a creature
        // (208.3); off the battlefield those printed on it or those a characteristic-defining
        // ability sets in every zone (604.3, 208.2a), and of a face-down spell the 2/2 the
        // ability that cast it face down says (708.2, 702.168b); an ability on the stack has
        // none (405.4); a planeswalker card's printed loyalty off the battlefield (306.5a)
        if let Some((power, toughness)) = o.pt {
            v["power"] = json!(power);
            v["toughness"] = json!(toughness);
        }
        if let Some(loyalty) = o.loyalty { v["loyalty"] = json!(loyalty); }
        if let Some(abilities) = &o.emblem {
            v["emblem"] = json!(true);
            v["abilities"] = json!(abilities.iter().map(|a| grammar::ability::abilities().ability.print_text(a, true)).collect::<Vec<_>>());
        }
        if o.commander { v["commander"] = json!(true); }
        // an exiled card an effect lets a player play, whether as a land too, and until when
        if let Some((player, play)) = o.may_play {
            v["may_play"] = json!({ "player": pid(player), "land": play });
            if let Some(until) = o.may_play_until { v["may_play"]["until"] = json!(until); }
            // grants with different rights or deadlines, each as it is (611.2a)
            if !o.may_play_grants.is_empty() {
                v["may_play"]["grants"] = json!(o.may_play_grants.iter().map(|(land, until)| json!({ "land": land, "until": until })).collect::<Vec<_>>());
            }
        }
        if let Some(turn) = o.plotted { v["plotted"] = json!({"turn": turn}); }
        // a warped card and the turn it was warped in (702.185a–b)
        if let Some(turn) = o.warped { v["warped"] = json!({"turn": turn}); }
        // a foretold card and the turn it was foretold, which is public (702.143e)
        if let Some(turn) = o.foretold { v["foretold"] = json!({"turn": turn}); }
        // a face-down permanent or spell (708.2, 708.4): how it came to be face down, which is
        // public, and its card only for a viewer who may look at it now (708.5)
        if let Some(down) = &o.face_down {
            v["face_down"] = json!({ "by": down.by.name(),
                "card": down.card.as_ref().map(|c| json!({ "name": c.name })) });
        }
        // a card exiled face down (406.3): who may look at it, which is public, and its card
        // only for a viewer who may look at it now; it has no name or types (406.3a). With a
        // watcher, the permanent whose controller may look at it as well (702.75a).
        if let Some(hidden) = &o.hidden {
            v["hidden"] = json!({ "lookers": hidden.lookers,
                "card": hidden.card.as_ref().map(|c| json!({ "name": c.name })) });
            if let Some(watcher) = hidden.watcher { v["hidden"]["watcher"] = oid(watcher); }
        }
        if let Some(p) = &o.permanent {
            v["attached_to"] = p.attached_to.as_ref().map(target).unwrap_or(Value::Null);
            v["tapped"] = json!(p.tapped);
            // a double-faced permanent with its back face up (701.27g)
            if p.transformed { v["transformed"] = json!(true); }
            v["damage"] = json!(p.damage);
            v["counters"] = json!(p.counters);
            v["sick"] = json!(p.sick);
            // a Class's level (716.2)
            if let Some(level) = p.level { v["level"] = json!(level); }
            // a Room's unlocked doors, the left first (709.5c)
            if let Some(unlocked) = p.unlocked { v["unlocked"] = json!(unlocked); }
            // its monstrous and renowned designations (701.37b, 702.112b)
            if p.monstrous { v["monstrous"] = json!(true); }
            if p.renowned { v["renowned"] = json!(true); }
            if p.solved { v["solved"] = json!(true); }
            if p.prepared { v["prepared"] = json!(true); }
            // saddled until end of turn (702.171b)
            if p.saddled { v["saddled"] = json!(true); }
            // which of its abilities are activated abilities, in the order printed
            v["activated"] = json!(p.activated);
            v["choices"] = json!(p.choices.iter().map(|r| json!({
                "group": r.link.0.origin.clone().name(), "producer": r.link.0.index,
                "chooser": r.chooser, "value": bound_value(&r.value)
            })).collect::<Vec<_>>());
        }
        if let Some(s) = &o.on_stack {
            v["ability"] = json!(s.ability);
            // the value announced for {X} in its cost, known to every player (107.3a)
            if let Some(x) = s.x { v["x"] = json!(x); }
            v["targets"] = json!(s.targets.iter().map(|group| group.iter().map(target).collect::<Vec<_>>()).collect::<Vec<_>>());
            // mana was spent on it that says it can't be countered (106.6)
            v["uncounterable"] = json!(s.uncounterable);
            // the opponent its gift was promised to as it was cast: the promise is public (702.174a)
            if let Some(opponent) = s.gift { v["gift"] = pid(opponent); }
            // the alternatives of its cost announced as it was proposed, public (601.2b)
            if !s.announced.is_empty() { v["announced"] = json!(s.announced); }
        }
        objects.insert(key(o.id), v);
    }
    let players: Vec<Value> = seen
        .players
        .iter()
        .enumerate()
        .map(|(player,p)| {
            let [w, u, b, r, green] = p.colors;
            // mana whose text restricts how it can be spent (106.6), one by one: its type, the
            // object whose text it is, whether the spell it is spent on can't be countered, and
            // until when it isn't lost as steps and phases end (`"kept"`: "combat" or "turn",
            // 106.4, 702.189a)
            let restricted: Vec<Value> = p
                .restricted
                .iter()
                .map(|m| {
                    let mut v = json!({ "mana": m.mana.text(), "source": oid(m.source), "from": m.from, "provenance": m.provenance, "uncounterable": m.uncounterable });
                    if let Some(kept) = m.kept { v["kept"] = json!(match kept { game::state::Kept::Combat => "combat", game::state::Kept::Turn => "turn" }); }
                    v
                })
                .collect();
            let mut v = json!({
                "life": p.life,
                "counters": p.counters,
                "hand_size": p.hand_size,
                "maximum_hand_size": p.maximum_hand_size,
                "library": p.library,
                "graveyard": ids(&p.graveyard),
                "pool": { "W": w, "U": u, "B": b, "R": r, "G": green, "C": p.colorless, "restricted": restricted },
                "land_played": p.land_played,
                "lost": p.lost,
            });
            if let Some(speed) = p.speed { v["speed"] = json!(speed); }
            if let Some(top)=p.top_card {v["top_card"]=oid(top);}
            if p.decision_maker != player { v["decision_maker"] = pid(p.decision_maker); }
            if labels() { v["label"] = pid(player); }
            if let Some((name, taken)) = &p.companion {
                v["companion"] = json!({"name":name,"taken":taken});
            }
            if !p.commanders.is_empty() {
                v["commanders"] = json!(p.commanders.iter().map(|c| json!({
                    "name": c.name, "identity": c.identity.iter().map(|c| format!("{c:?}")).collect::<Vec<_>>(),
                    "casts": c.casts, "damage": c.damage,
                })).collect::<Vec<_>>());
            }
            if let Some(hand) = &p.hand {
                v["hand"] = json!(ids(hand));
            }
            v
        })
        .collect();
    let mut state = json!({
        "viewer": viewer,
        "turn": seen.turn,
        "monarch": seen.monarch.map(pid),
        "part": format!("{:?}", seen.part),
        "active": pid(seen.active),
        "priority": pid(seen.priority),
        "winner": seen.winner.map(pid),
        "finished": seen.finished,
        // the mulligans and opening-hand actions precede the first turn (103.8)
        "begun": seen.begun,
        "choices_made": seen.choices_made.iter().map(|c| match c {
            game::state::PublicChoice::OrderCards { player, to, cards } =>
                json!({"player":pid(*player),"order_cards":{"to":placement(*to),"cards":ids(cards)}}),
            game::state::PublicChoice::LibraryDestination { player, cards, position } =>
                json!({"player":pid(*player),"cards":ids(cards),"library_position":placement(game::state::Placement::Library(*position))}),
            game::state::PublicChoice::Alternative { player, source, ability, instruction, index } =>
                json!({ "player": pid(*player), "source": oid(*source),
                    "ability": ability.clone().map(|a| json!({ "group": a.origin.name(), "index": a.index })),
                    "instruction": instruction, "alternative": index }),
            game::state::PublicChoice::Value { player, source, ability, slot, value } =>
                json!({ "player": pid(*player), "source": oid(*source),
                    "ability": ability.clone().map(|a| json!({ "group": a.origin.name(), "index": a.index })),
                    "slot": slot, "value": bound_value(value) }),
            game::state::PublicChoice::Objects { player, verb, objects } =>
                json!({ "player": pid(*player), "verb": format!("{verb:?}"), "objects": ids(objects) }),
            game::state::PublicChoice::Entry { player, card, answer } => {
                let answer = match answer {
                    Answer::Value(value) => json!({ "value": bound_value(value) }),
                    Answer::Yes => json!({ "yes": true }),
                    Answer::No => json!({ "yes": false }),
                    Answer::Attach(to) => json!({ "attach": target(to) }),
                    Answer::Objects(objects) => json!({ "objects": ids(objects) }),
                    Answer::Defender(to) => json!({ "defender": target(to) }),
                    _ => unreachable!("entry observations contain values, consent, attachment, copy or defender selection"),
                };
                json!({ "player": pid(*player), "entering": oid(*card), "answer": answer })
            }
            game::state::PublicChoice::SpellDestination{player,on,effect}=>json!({"player":pid(*player),"spell_destination":{"on":oid(*on),"effect":effect.name()}}),
            game::state::PublicChoice::DamageOrder{player,on,offer}=>json!({"player":pid(*player),"on":target(on),"damage_order":damage_option(offer)}),
            game::state::PublicChoice::Prevention{player,on,shield,allocation}=>json!({"player":pid(*player),"on":target(on),"shield":replacement(shield),"prevention":allocation}),
            game::state::PublicChoice::DamageReplacement {player,on,offer} =>
                json!({"player":pid(*player),"on":target(on),"replacement":damage_offer(offer),"damage_source":oid(offer.source)}),
            game::state::PublicChoice::Replacement { player, on, effect } =>
                json!({ "player": pid(*player), "on": target(on), "replacement": replacement(effect) }),
        }).collect::<Vec<_>>(),
        "players": players,
        "battlefield": ids(&seen.battlefield),
        "exile": ids(&seen.exile),
        "command": ids(&seen.command),
        "stack": ids(&seen.stack),
        "attacking": ids(&seen.attacking),
        // what each attacks, declared or put onto the battlefield attacking (506.4, 508.4)
        "attacks": seen.attacks.iter().map(|(attacker, at)| json!({ "attacker": oid(*attacker), "target": target(at) })).collect::<Vec<_>>(),
        "objects": objects,
    });
    if !seen.publicly_revealed.is_empty() {
        state["publicly_revealed"] = json!(ids(&seen.publicly_revealed));
    }
    // said only of a turn an effect changed, so that an ordinary game reads as it did
    if seen.turn_modified {
        if let Some((id, added)) = seen.phase {
            state["phase"] = json!({ "id": id.0, "added": added });
        }
        state["upcoming"] = json!(seen.upcoming.iter().map(|(id, phase, added)| json!({ "id": id.0, "phase": format!("{phase:?}"), "added": added })).collect::<Vec<_>>());
    }
    if !seen.skips.is_empty() {
        state["skips"] = json!(seen.skips.iter().map(skip_effect).collect::<Vec<_>>());
    }
    if let Some(what) = seen.skipping {
        state["skipping"] = occurrence(what);
    }
    state
}

/// Whom a request's answer is shown to: `"viewer":0` that player, `"viewer":"asked"` the
/// player the game is waiting for (everyone, once it is over), and with none said, all of it.
/// It is read before the request is carried out, so that one naming no viewer is refused
/// having changed nothing: who the players are does not change as the game goes on.
#[derive(Debug, Clone, Copy)]
enum Viewer { Whole, Asked, Seat(usize) }

fn read_viewer(g: &Game, request: &Value) -> Result<Viewer, Error> {
    match request.get("viewer") {
        None | Some(Value::Null) => Ok(Viewer::Whole),
        Some(Value::String(s)) if s == "asked" => Ok(Viewer::Asked),
        Some(v) => match v.as_u64() {
            Some(p) if (p as usize) < g.players.len() => Ok(Viewer::Seat(p as usize)),
            // a number is a player's, but the game has no such player
            Some(_) => Err(Error::unreferable(format!("a viewer is a player or \"asked\", not {v}"))),
            None => Err(format!("a viewer is a player or \"asked\", not {v}").into()),
        },
    }
}

impl Viewer {
    fn seat(self, chooser: Option<usize>) -> Option<usize> {
        match self { Self::Whole => None, Self::Asked => chooser, Self::Seat(seat) => Some(seat) }
    }
}

fn read_target(v: &Value) -> Result<Target, String> {
    if let Some(label) = v.as_str() {
        return match Label::parse(label) {
            Some(Label::Player(p)) => Ok(Target::Player(p)),
            _ => match unnamed(label) {
                Some((o, None)) => Ok(Target::Object(o)),
                _ => Err(format!("a target is a player or an object label, not {v}")),
            },
        };
    }
    if let Some(p) = v.get("player").and_then(Value::as_u64) {
        Ok(Target::Player(p as usize))
    } else if let Some(o) = v.get("object").and_then(Value::as_u64) {
        Ok(Target::Object(ObjId(o)))
    } else {
        Err(format!("a target is {{\"player\":n}} or {{\"object\":n}}, not {v}"))
    }
}

fn read_id(v: &Value) -> Result<ObjId, String> {
    if let Some(label) = v.as_str() {
        return match unnamed(label) { Some((o, None)) => Ok(o), _ => Err(format!("an object label, not {v}")) };
    }
    v.as_u64().map(|o| ObjId(o)).ok_or_else(|| format!("an object id, not {v}"))
}

/// The ability a move names: `"ability":"o42.a3"`, its label, or `"source":42,"ability":3`.
fn read_ability(v: &Value) -> Result<(ObjId, usize), String> {
    match v.get("ability") {
        Some(Value::String(label)) => match unnamed(label) {
            Some((source, Some(index))) if v.get("source").is_none_or(|s| read_id(s) == Ok(source)) => Ok((source, index)),
            _ => Err("an ability is an ability label (\"o12.a3\") of the source named".to_string()),
        },
        Some(index) => Ok((read_id(v.get("source").ok_or("the move has no \"source\"")?)?,
            index.as_u64().ok_or("an ability is a label or an index")? as usize)),
        None => Err("the move has no \"ability\"".into()),
    }
}

fn list<'a>(v: &'a Value, field: &str) -> &'a [Value] {
    v.get(field).and_then(Value::as_array).map_or(&[], |a| a.as_slice())
}

fn read_groups(v: &Value, field: &str) -> Result<Vec<Vec<Target>>, String> {
    list(v, field).iter().map(|group| group.as_array().into_iter().flatten().map(read_target).collect()).collect()
}

/// A move, as `scripts/compare.py` says it:
/// `{"kind":"pass"}`, `{"kind":"play","card":7}`, `{"kind":"mana","permanent":3}`,
/// `{"kind":"cast","card":7,"targets":[[{"player":1}]]}` (of a modal spell with `"modes":[0]`),
/// `{"kind":"activate","source":3,"ability":0,"targets":[…],"paying":[[5]]}`,
/// `{"kind":"targets","targets":[…]}` (of a modal triggered ability with `"modes":[1]`),
/// `{"kind":"discard","cards":[7]}`,
/// `{"kind":"attack","attacks":[[4,{"player":1}]]}`, `{"kind":"block","blocks":[[9,4]]}`,
/// `{"kind":"divide","division":[[{"object":9},2]]}`,
/// `{"kind":"choose","answer":"yes"}` (or `"no"`, or `{"objects":[5]}`, `{"mana":["R"]}`,
/// `{"value":{"creature_type":"Goblin"}}`, `{"value":{"color":"red"}}`, `{"top":[5],"away":[6]}`,
/// `{"pay":{"plan":[…],"mana":{"B":1}}}` to a question of kind `"pay"`, which requires
/// explicit payment instructions or a decline). A cast or an activation with `"mana":{"R":1,"C":2}` pays the
/// mana of its cost with that mana of the pool, and with `"restricted":[0]` in it also with
/// the restricted mana of the pool at those places of its list (`pool`: the mover's pool).
fn replacement_ability(key: game::binding::AbilityKey) -> Value {
    use game::binding::Origin;
    match key.origin {
        Origin::Own => json!(key.index),
        Origin::Inherited(family) => json!({"index": key.index, "origin": "inherited", "family": family.identity()}),
        Origin::Copied(stamp) => json!({"index": key.index, "origin": "copied", "stamp": stamp}),
        Origin::Given { grant, what } => json!({"index": key.index, "origin": "given",
            "grant": {"source": grant.by.map(oid), "stamp": grant.stamp, "part": grant.part},
            "text": grammar::ability::abilities().ability.print_text(what.ability(), true)}),
        Origin::Rule(what) => json!({"index": key.index, "origin": "rule", "text": grammar::ability::abilities().ability.print_text(what.ability(), true)}),
    }
}

/// The names a cast says it pays its optional additional cost by ([`game::keyword::Optional::name`]).
const OPTIONAL_COSTS: [&str; 5] = ["bargain", "additional", "offspring", "casualty", "teamwork"];

/// What a cast casts the card as: the whole card, a split card's half or both halves fused
/// (709.3, 702.102), its Adventure or Omen (715.3, 720.3), or a modal face (712.11b).
fn read_cast_as(choice: &Value) -> Result<CastAs, String> {
    Ok(match choice {
            v if v.get("face").is_some() => CastAs::Face(v["face"].as_u64().ok_or("a modal face index")? as usize),
            Value::String(s) if s == "whole" => CastAs::Whole,
            Value::String(s) if s == "fused" => CastAs::Fused,
            Value::String(s) if s == "adventure" => CastAs::Adventure,
            Value::String(s) if s == "omen" => CastAs::Omen,
            n if n.as_u64().is_some() => CastAs::Half(n.as_u64().unwrap() as usize),
            _ => return Err(r#"cast_as is "whole", a half's index, "fused", "adventure", "omen" or {"face":n}"#.into()),
        })
}

/// A cast says its optional additional cost by what that cost is: `"teamwork"` for a spell
/// whose cost is offspring is a client's mistake, refused rather than read as the other cost.
fn optional_named(v: &Value, g: &game::Game) -> Result<(), Error> {
    let names: Vec<_> = OPTIONAL_COSTS.into_iter().filter(|name| v.get(*name).is_some()).collect();
    if names.len() > 1 { return Err("a cast names one optional additional cost".into()); }
    let Some(name) = names.first() else { return Ok(()) };
    let Some(card) = v.get("card").and_then(|c| read_id(c).ok()) else { return Ok(()) };
    let face = v.get("cast_as").map(read_cast_as).transpose()?.unwrap_or_default();
    if g.get_object(card).is_none() { return Err(Error::unreferable("an object handle is stale or unknown")); }
    let def = g.def(card).ok_or_else(|| Error::illegal("not a card"))?.spell(face).map_err(Error::illegal)?;
    match def.optional_cost_kind() {
        Some(kind) if kind.name() == *name => Ok(()),
        Some(kind) => Err(Error::illegal(format!("that spell's optional additional cost is {}, not {name} (601.2b)", kind.name()))),
        None => Err(Error::illegal("that spell has no optional additional cost (118.8b)")),
    }
}

/// Literal test-fixture spending, with every unit authored by the test.
#[cfg(test)]
fn paid(mana: &str, action: Move) -> Move {
    let mut pool = Pool::default();
    for unit in mana.chars() {
        match unit { 'W'=>pool.colors[0]+=1, 'U'=>pool.colors[1]+=1, 'B'=>pool.colors[2]+=1,
            'R'=>pool.colors[3]+=1, 'G'=>pool.colors[4]+=1, 'C'=>pool.colorless+=1,
            _=>panic!("a literal mana unit"), }
    }
    Move::Paying(pool, Box::new(action))
}

#[cfg(test)]
fn read_move(v: &Value, pool: &Pool) -> Result<Move,Error> { read_move_offered(v, pool, &[]) }

/// The keys a move of each kind may say: those read, and those its listing shows that are
/// sent back with it unread (`slots`, `pay_life`, `face_down`, `asks`). Any other is refused,
/// so a misspelled field ("face" for "cast_as") can't make another move than the one meant.
fn move_keys(kind: &str) -> Option<&'static [&'static str]> {
    const PAYS: [&str; 2] = ["plan", "mana"];
    Some(match kind {
        "pass" | "concede" | "mulligan" | "keep" | "reverse_payment" => &[],
        "play" => &["card", "face"],
        "plot" | "foretell" => &["card", PAYS[0], PAYS[1]],
        "companion" => &PAYS,
        "turn_face_up" => &["permanent", "way", PAYS[0], PAYS[1]],
        "unlock" => &["room", "half", PAYS[0], PAYS[1]],
        "cast" => &["card", "targets", "paying", "convoke", "cast_as", "modes", "x", "replicate", "entwine", "buyback", "bargain", "additional", "offspring", "casualty", "teamwork", "kicks", "cost",
            "delve", "route", "gift", "life", "chose", "alternatives", "cost_branches", PAYS[0], PAYS[1], "slots", "pay_life", "face_down", "division"],
        "activate" => &["source", "ability", "targets", "paying", "modes", "adding", "x", "life", "chose", "alternatives", "cost_branches", PAYS[0], PAYS[1], "slots", "asks", "division"],
        "payment" => &["paying", PAYS[0], PAYS[1]],
        "targets" => &["targets", "modes", "division"],
        "modes" => &["modes"],
        "order" => &["order"],
        "discard" | "bottom" => &["cards"],
        "attack" => &["attacks"],
        "block" => &["blocks"],
        "divide" => &["division"],
        "choose" => &["pending_payment", "answer"],
        _ => return None,
    })
}

/// The keys a `choose` answer object may say: one answer's, or an arrangement's two.
const ANSWER_KEYS: &[&str] = &["call", "library_position", "spell_destination", "targets", "objects", "accept", "player", "alternative", "skip", "pile", "prevention", "holders",
    "attach", "defender", "mana", "value", "replacement", "creature_type", "top", "away", "pay"];

fn read_library_position(value: &Value) -> Result<ir::action::LibraryPosition, String> {
    use ir::action::LibraryPosition;
    let fields = value.as_object().ok_or("library_position: a position object")?;
    if fields.keys().any(|key| !["zone", "position", "from_top"].contains(&key.as_str()))
        || fields.get("zone").is_some_and(|zone| zone != "library") {
        return Err("library_position: only library zone and one position are allowed".into());
    }
    match (fields.get("position"), fields.get("from_top")) {
        (Some(position), None) if position == "top" => Ok(LibraryPosition::Top),
        (Some(position), None) if position == "bottom" => Ok(LibraryPosition::Bottom),
        (None, Some(n)) => n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n > 0)
            .map(LibraryPosition::FromTop).ok_or("library_position: from_top must be a positive u32".into()),
        _ => Err("library_position: choose top, bottom, or from_top".into()),
    }
}

fn known_keys(v: &Value) -> Result<(), String> {
    let kind = v.get("kind").and_then(Value::as_str).unwrap_or("");
    let Some(keys) = move_keys(kind) else { return Ok(()) };
    if let Some(unknown) = v.as_object().into_iter().flatten().map(|(k, _)| k.as_str()).find(|k| *k != "kind" && !keys.contains(k)) {
        return Err(format!("a {kind} move has no \"{unknown}\"; it says {}", if keys.is_empty() { "nothing more".into() } else { keys.join(", ") }));
    }
    if let Some(answer) = v.get("answer").and_then(Value::as_object).filter(|_| kind == "choose") {
        if let Some(unknown) = answer.keys().find(|k| !ANSWER_KEYS.contains(&k.as_str())) {
            return Err(format!("an answer has no \"{unknown}\"; it says one of {}", ANSWER_KEYS.join(", ")));
        }
        let arrangement = answer.keys().all(|k| k == "top" || k == "away");
        let optional_objects = answer.contains_key("accept") && answer.keys().all(|k| k == "accept" || k == "objects");
        if answer.len() > 1 && !arrangement && !optional_objects {
            return Err(format!("an answer says one thing, not {}", answer.keys().map(String::as_str).collect::<Vec<_>>().join(" and ")));
        }
    }
    Ok(())
}

/// Decode an exact pool selection without coercing malformed counts or losing units.
fn read_pool_selection(mana: &Value, pool: &Pool) -> Result<Pool,Error> {
    let fields = mana.as_object().ok_or("mana selection is an object")?;
    if fields.keys().any(|k| !["W", "U", "B", "R", "G", "C", "restricted"].contains(&k.as_str())) {
        return Err("mana selection names W U B R G C counts and restricted indices; post-plan indices require a quoted boundary".into());
    }
    let count = |letter: &str| -> Result<u32, String> {
        match fields.get(letter) {
            None => Ok(0),
            Some(value) => value.as_u64().and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| format!("mana {letter} is a nonnegative u32 count")),
        }
    };
    if let Some(indices) = fields.get("restricted").and_then(Value::as_array) {
        if indices.iter().enumerate().any(|(i, value)| indices[..i].contains(value)) {
            return Err("restricted mana indices must be distinct".into());
        }
    }
    let restricted = match fields.get("restricted") {
        None => Vec::new(),
        Some(value) => value.as_array().ok_or("restricted mana selection is a list of indices")?.iter()
            .map(|i| i.as_u64().and_then(|n| usize::try_from(n).ok()).and_then(|i| pool.restricted.get(i)).cloned()
                .ok_or_else(|| Error::unreferable("no such restricted mana in the selected pool boundary")))
            .collect::<Result<Vec<_>, _>>()?,
    };
    Ok(Pool { colors: [count("W")?, count("U")?, count("B")?, count("R")?, count("G")?], colorless: count("C")?, restricted })
}

fn read_move_offered(v: &Value, pool: &Pool, offered: &[game::operation::ReplacementId]) -> Result<Move,Error> {
    read_move_at_pools(v, pool, offered, &std::collections::BTreeMap::new())
}

fn read_move_at_pools(v: &Value, pool: &Pool, offered: &[game::operation::ReplacementId], step_pools: &std::collections::BTreeMap<Vec<usize>,Pool>) -> Result<Move,Error> {
    known_keys(v)?;
    if v["kind"] == "payment" {
        let mut request = v.clone();
        request["kind"] = json!("cast");
        request["card"] = json!(0);
        let mut action = read_move_at_pools(&request, pool, offered, step_pools)?;
        let (mut plan, mut paying, mut with) = (Vec::new(), Vec::new(), None);
        loop {
            action = match action {
                Move::WithMana(steps, inner) => { plan = steps; *inner },
                Move::WithCosts(costs, inner) => { paying = costs; *inner },
                Move::Paying(mana, inner) => { with = Some(mana); *inner },
                Move::Cast { targets, .. } if targets.is_empty() => return Ok(Move::FinishPayment { plan, paying, with }),
                _ => return Err("a payment correction supplies only plan, paying and mana".into()),
            };
        }
    }
    // "You may pay {B}" (118.12): a payment said as one of a spell is; its objects, those
    // tapped for a waterbend cost (701.67a), as one group
    if let Some(pay) = v.get("answer").and_then(|a| a.get("pay")).filter(|_| v["kind"] == "choose") {
        let mut request = pay.clone();
        if !request.is_object() { return Err("a payment is an object with plan and mana".into()); }
        request["kind"] = json!("payment");
        return match read_move_at_pools(&request, pool, offered, step_pools)? {
            Move::FinishPayment { plan, mut paying, with } if paying.len() <= 1 => Ok(Move::Choose(Answer::Pay { plan, with, tapped: paying.pop().unwrap_or_default() })),
            _ => Err("an effect's payment supplies only plan, mana and the objects tapped for a waterbend cost".into()),
        };
    }
    if v["kind"] == "modes" {
        let modes = v["modes"].as_array().ok_or("modes is a list")?.iter()
            .map(|n| n.as_u64().and_then(|n| usize::try_from(n).ok()).ok_or_else(|| "a mode is a nonnegative index".to_owned()))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Move::ChooseModes(modes));
    }
    let mut mv = read_plain(v, offered)?;
    if v["kind"] == "cast" && v.get("paying").is_some() {
        let paying = v["paying"].as_array().ok_or("paying is a list of payment groups")?.iter().map(|group| group.as_array().ok_or("a payment is a list")?.iter().map(read_id).collect()).collect::<Result<_, String>>()?;
        mv = Move::WithCosts(paying, Box::new(mv));
    }
    if let Some(allocation)=v.get("convoke") {
        let allocation=allocation.as_array().ok_or("convoke is a list of colors or generic")?.iter().map(|c| match c.as_str() {
            Some("generic")=>Ok(None),Some("W")=>Ok(Some(ir::Color::White)),Some("U")=>Ok(Some(ir::Color::Blue)),Some("B")=>Ok(Some(ir::Color::Black)),Some("R")=>Ok(Some(ir::Color::Red)),Some("G")=>Ok(Some(ir::Color::Green)),_=>Err("a convoke color or generic")
        }).collect::<Result<Vec<_>,_>>()?;
        mv=Move::WithConvoke(allocation,Box::new(mv));
    }
    if let Some(choice) = v.get("cast_as") {
        let choice = read_cast_as(choice)?;
        mv = Move::CastAs(choice, Box::new(mv));
    }
    // a cast, an activation or the targets of a triggered ability with `"modes":[0,2]`: of a
    // modal spell or ability, the modes chosen (700.2)
    if let Some(modes) = v.get("modes").and_then(Value::as_array).filter(|m| !m.is_empty() || v["kind"] == "targets") {
        let modes = modes.iter().map(|m| m.as_u64().map(|m| m as usize).ok_or("a mode is a place among the modes")).collect::<Result<_, _>>()?;
        mv = Move::Modes(modes, Box::new(mv));
    }
    // of targets among which an amount is divided (a slot's "divide"), with `"division":[[2,1]]`:
    // what each target of each group gets, in the order the targets are said (601.2d)
    if let Some(division) = v.get("division").filter(|d| !d.is_null() && v["kind"] != "divide") {
        let groups = division.as_array().ok_or("division is a list of groups of amounts (601.2d)")?;
        let division = groups.iter().map(|g| g.as_array().ok_or("a group's division is a list of amounts (601.2d)")?.iter()
            .map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or("an amount of a division is a whole number (601.2d)"))
            .collect::<Result<Vec<u32>, _>>()).collect::<Result<_, _>>()?;
        mv = Move::WithDivision(division, Box::new(mv));
    }
    // an activation of a mana ability with `"adding":[["R"]]` adds that mana, where its text
    // offers a choice
    if let Some(adding) = v.get("adding").and_then(Value::as_array).filter(|a| !a.is_empty()) {
        mv = Move::Adding(adding.iter().map(read_mana).collect::<Result<_, _>>()?, Box::new(mv));
    }
    // a cast or an activation whose cost has {X} with `"x":3` announces that value (107.3a);
    // `"x":null`, as a listing without {X} says it, announces none
    if let Some(x) = v.get("x").filter(|x| !x.is_null()) {
        if v["kind"] != "cast" && v["kind"] != "activate" { return Err("x is announced with a cast or an activation (107.3a)".into()); }
        let x = x.as_u64().and_then(|x| u32::try_from(x).ok()).ok_or("x is a whole number that is not negative (107.1b)")?;
        mv = Move::WithX(x, Box::new(mv));
    }
    if let Some(times) = v.get("replicate").filter(|v| !v.is_null()) {
        if v["kind"] != "cast" { return Err("replicate is announced with a cast".into()); }
        let times = times.as_u64().and_then(|v| u32::try_from(v).ok()).ok_or("replicate is a nonnegative whole number")?;
        mv = Move::WithReplicate(times, Box::new(mv));
    }
    if let Some(paid) = v.get("entwine") {
        if v["kind"] != "cast" { return Err("entwine is announced with a cast (702.42)".into()); }
        mv = Move::WithEntwine(paid.as_bool().ok_or("entwine must be a boolean")?, Box::new(mv));
    }
    // a spell's optional additional cost, by what it is: `"bargain":true`, `"offspring"`,
    // `"casualty"` or, for the cost its text prints, `"additional"` (601.2b)
    for name in OPTIONAL_COSTS {
        let Some(paid) = v.get(name) else { continue };
        if v["kind"] != "cast" { return Err(format!("{name} is announced with a cast (601.2b)").into()); }
        mv = Move::WithOptional(paid.as_bool().ok_or(format!("{name} must be a boolean"))?, Box::new(mv));
    }
    if let Some(paid) = v.get("buyback") {
        if v["kind"] != "cast" { return Err("buyback is announced with a cast (702.27a)".into()); }
        mv = Move::WithBuyback(paid.as_bool().ok_or("buyback must be a boolean")?, Box::new(mv));
    }
    // a cast with `"kicks":2` declares its kicker cost paid that many times (601.2b, 702.33c)
    if let Some(kicks) = v.get("kicks").filter(|k| !k.is_null()) {
        if v["kind"] != "cast" { return Err("a kicker cost is paid with a cast (702.33a)".into()); }
        let kicks = kicks.as_u64().and_then(|k| u32::try_from(k).ok()).ok_or("kicks is a whole number that is not negative (107.1b)")?;
        mv = Move::WithKicks(kicks, Box::new(mv));
    }
    // a cast with `"cost":{"alternative":0}` is cast for the alternative cost its card prints
    // in that row rather than its mana cost (118.9); a flashback route can select one of
    // its effective costs with `"cost":{"flashback":0}` (113.2c).
    if let Some(cost) = v.get("cost").filter(|c| !c.is_null()) {
        if v["kind"] != "cast" { return Err("the cost a spell is cast for is said with a cast (601.2b)".into()); }
        let cost = match cost {
            Value::String(s) if s == "mana" => game::announcement::CastCost::Mana,
            Value::Object(object) if object.len() == 1 && object.contains_key("flashback") =>
                game::announcement::CastCost::Flashback(object["flashback"].as_u64()
                    .and_then(|n| usize::try_from(n).ok()).ok_or("a flashback cost is a nonnegative cost index")?),
            Value::Object(object) if object.len() == 1 && object.contains_key("alternative") =>
                game::announcement::CastCost::Alternative(object["alternative"].as_u64()
                    .and_then(|n| usize::try_from(n).ok()).ok_or("an alternative cost is a nonnegative ability row")?),
            _ => return Err("a cost is \"mana\", {\"alternative\": an ability row}, or {\"flashback\": a cost index}".into()),
        };
        mv = Move::CastFor(cost, Box::new(mv));
    }
    if let Some(delve) = v.get("delve").filter(|v| !v.is_null()) {
        if v["kind"] != "cast" { return Err("delve is a casting payment (702.66)".into()); }
        let count = delve.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or("delve is a nonnegative card count")?;
        mv = Move::WithDelve(count, Box::new(mv));
    }
    if let Some(route) = v.get("route").filter(|r| !r.is_null()) {
        if v["kind"] != "cast" { return Err("a casting route is said with a cast (601.3)".into()); }
        let route = match route.as_str() {
            Some("normal") => game::announcement::CastRoute::Normal,
            Some("flashback") => game::announcement::CastRoute::Flashback,
            Some("retrace") => game::announcement::CastRoute::Retrace,
            Some("escape") => game::announcement::CastRoute::Escape,
            Some("foretell") => game::announcement::CastRoute::Foretell,
            // the creature tapped as its harmonize cost is paid, chosen as it is cast (702.180b)
            Some("harmonize") => game::announcement::CastRoute::Harmonize { tapped: None },
            None if route.get("harmonize").is_some() => game::announcement::CastRoute::Harmonize { tapped: Some(read_id(&route["harmonize"])?) },
            None if route.is_object() => game::announcement::CastRoute::Permission {source:read_id(&route["source"])?,ability:route["ability"].as_u64().ok_or("permission ability index")? as usize},
            _ => return Err("a casting route is normal, flashback, retrace, escape, foretell, harmonize ({\"harmonize\": a creature to tap}) or a permission source and ability".into()),
        };
        mv = Move::CastVia(route, Box::new(mv));
    }
    // `"gift": "p1"`: the opponent a spell's gift is promised to, by player label (702.174a);
    // its seat number, as recordings said it, is read too; absent or null, none
    if let Some(gift) = v.get("gift").filter(|g| !g.is_null()) {
        if v["kind"] != "cast" { return Err("a gift is promised as a spell is cast (702.174a)".into()); }
        let opponent = match gift {
            Value::String(label) => match Label::parse(label) { Some(Label::Player(p)) => Some(p), _ => None },
            n => n.as_u64().map(|p| p as usize),
        }.ok_or_else(|| {
            // an object's label is already read as the object it names, which the player never saw
            let said = if gift.as_str().is_some_and(|g| g.starts_with(UNNAMED)) { "an object label".to_owned() } else { gift.to_string() };
            format!("a gift is promised to an opponent, by player label (\"p1\"), not {said}")
        })?;
        mv = Move::GiftTo(opponent, Box::new(mv));
    }
    // `"life":[true,false]`: for each Phyrexian symbol of the cost, whether 2 life is paid
    // for it (107.4f, 601.2b); absent or null pays mana for all
    if let Some(life) = v.get("life").filter(|l| !l.is_null()) {
        if v["kind"] != "cast" && v["kind"] != "activate" { return Err("life for Phyrexian mana is said with a cast or an activation (601.2b)".into()); }
        let life = life.as_array().ok_or("life is a list of true or false, one for each Phyrexian symbol")?.iter()
            .map(|b| b.as_bool().ok_or("life is a list of true or false, one for each Phyrexian symbol")).collect::<Result<Vec<_>, _>>()?;
        mv = Move::WithLife(life, Box::new(mv));
    }
    // `"chose":[1]`: for each choice among alternatives of the cost, which alternative is paid,
    // numbered from 0 as the listing's `"alternatives"` says them (601.2b, 602.2b)
    if let Some(chose) = v.get("chose").filter(|c| !c.is_null()) {
        if v["kind"] != "cast" && v["kind"] != "activate" { return Err("the alternative of a cost paid is said with a cast or an activation (601.2b)".into()); }
        let chose = chose.as_array().ok_or("chose is a list of alternatives, one number for each choice of the cost")?.iter()
            .map(|i| i.as_u64().map(|i| i as usize).ok_or("chose is a list of alternatives, one number for each choice of the cost")).collect::<Result<Vec<_>, _>>()?;
        mv = Move::WithCostChoice(chose, Box::new(mv));
    }
    if let Some(branches)=v.get("cost_branches") {
        if v["kind"]!="cast" && v["kind"]!="activate" {return Err("cost branches require a cast or activation".into());}
        mv=Move::WithCostBranches(serde_json::from_value(branches.clone()).map_err(|error|format!("invalid cost branches: {error}"))?,Box::new(mv));
    }
    if let Some(plan) = v.get("plan") {
        let steps = read_payment_steps(plan, pool, step_pools, &[])?;
        mv = Move::WithMana(steps, Box::new(mv));
    }
    if let Some(mana) = v.get("mana") {
        mv = Move::Paying(read_pool_selection(mana, pool)?, Box::new(mv));
    }
    Ok(mv)
}

fn read_payment_steps(v:&Value, pool:&Pool, pools:&std::collections::BTreeMap<Vec<usize>,Pool>, parent:&[usize]) -> Result<Vec<game::legal::Step>,Error> {
    v.as_array().ok_or("a mana plan is a list of activations")?.iter().enumerate().map(|(index,s)| {
        let mut path=parent.to_vec();path.push(index);
        Ok(game::legal::Step {
            source:read_ability(s)?.0,ability:read_ability(s)?.1,
            plan:s.get("plan").map(|v|read_payment_steps(v,pool,pools,&path)).transpose()?.unwrap_or_default(),
            x:s.get("x").map(|v|v.as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("step X must be a nonnegative integer")).transpose()?,
            life:list(s,"life").iter().map(|v|v.as_bool().ok_or("step life must contain booleans")).collect::<Result<_,_>>()?,
            with:s.get("mana").map(|v|read_pool_selection(v,pools.get(&path).unwrap_or(pool))).transpose()?,
            paying:list(s,"paying").iter().map(|g|g.as_array().ok_or("a payment is a list")?.iter().map(read_id).collect()).collect::<Result<_,String>>()?,
            adding:list(s,"adding").iter().map(read_mana).collect::<Result<_,_>>()?,
            chose:list(s,"chose").iter().map(|v|v.as_u64().map(|n|n as usize).ok_or("step cost choice must be an integer")).collect::<Result<_,_>>()?,
            cost_branches:s.get("cost_branches").map(|v|serde_json::from_value(v.clone()).map_err(|e|format!("invalid mana cost branches: {e}"))).transpose()?,
        })
    }).collect()
}

fn read_plain(v: &Value, offered: &[game::operation::ReplacementId]) -> Result<Move,Error> {
    let id = |field: &str| v.get(field).ok_or_else(|| format!("the move has no \"{field}\"")).and_then(read_id);
    Ok(match v.get("kind").and_then(Value::as_str).unwrap_or("") {
        "pass" => Move::Pass,
        // any player, at any time (104.3a)
        "concede" => Move::Concede,
        "play" => match v.get("face") {None=>Move::PlayLand(id("card")?),Some(face)=>Move::PlayLandAs{card:id("card")?,face:face.as_u64().ok_or("a modal face index")? as usize}},
        "plot" => Move::Plot(id("card")?),
        "foretell" => Move::Foretell(id("card")?),
        "turn_face_up" => Move::TurnFaceUp(id("permanent")?, {
            let way = v.get("way").and_then(Value::as_str);
            *game::face::Way::ALL.iter().find(|w| Some(w.name()) == way)
                .ok_or("a turn_face_up move says its \"way\": \"manifested\", \"cloaked\", \"morph\" or \"disguise\"")?
        }),
        "unlock" => Move::Unlock { room: id("room")?, half: v.get("half").and_then(Value::as_u64).filter(|h| *h < 2).ok_or("an unlock move says its \"half\": 0 or 1")? as usize },
        "companion" => Move::Companion,
        "cast" => Move::Cast { card: id("card")?, targets: read_groups(v, "targets")? },
        "activate" => Move::Activate {
            source: read_ability(v)?.0,
            ability: read_ability(v)?.1,
            targets: read_groups(v, "targets")?,
            paying: list(v, "paying").iter().map(|group| group.as_array().into_iter().flatten().map(read_id).collect()).collect::<Result<_, _>>()?,
        },
        "targets" => Move::ChooseTargets(read_groups(v, "targets")?),
        "order" => Move::Order(list(v, "order").iter().map(|i| i.as_u64().map(|i| i as usize).ok_or("an order is a list of places")).collect::<Result<_, _>>()?),
        "discard" => Move::Discard(list(v, "cards").iter().map(read_id).collect::<Result<_, _>>()?),
        "mulligan" => Move::Mulligan,
        "keep" => Move::Keep,
        "bottom" => Move::Bottom(list(v, "cards").iter().map(read_id).collect::<Result<_, _>>()?),
        "attack" => Move::Attack(
            list(v, "attacks")
                .iter()
                .map(|pair| Ok((read_id(&pair[0])?, read_target(&pair[1])?)))
                .collect::<Result<_, String>>()?,
        ),
        "block" => Move::Block(list(v, "blocks").iter().map(|pair| Ok((read_id(&pair[0])?, read_id(&pair[1])?))).collect::<Result<_, String>>()?),
        "divide" => Move::Divide(
            list(v, "division")
                .iter()
                .map(|pair| Ok((read_target(&pair[0])?, pair[1].as_u64().ok_or("an amount")? as u32)))
                .collect::<Result<_, String>>()?,
        ),
        "reverse_payment" => Move::ReversePayment,
        "choose" => Move::Choose(match v.get("answer") {
            Some(Value::String(s)) if s == "yes" => Answer::Yes,
            Some(Value::String(s)) if s == "no" => Answer::No,
            Some(a) if a.get("spell_destination").is_some()=>Answer::SpellDestination {
                on: read_id(&a["spell_destination"]["on"])?,
                effect: game::state::DestinationEffect::from_name(a["spell_destination"]["effect"].as_str().ok_or("spell_destination: named effect")?).ok_or("spell_destination: unknown effect")?,
            },
            Some(a) if a.get("targets").is_some() => Answer::Targets(list(a, "targets").iter().map(|group| group.as_array().ok_or_else(|| "target group must be an array".to_string())?.iter().map(read_target).collect::<Result<Vec<_>, String>>()).collect::<Result<Vec<_>, String>>()?),
            Some(a) if a.get("accept").is_some() => {
                let accept = a["accept"].as_bool().ok_or("optional objects: accept must be a boolean")?;
                if accept {
                    let objects = a["objects"].as_array().ok_or("optional objects: acceptance requires objects")?;
                    Answer::OptionalObjects(Some(objects.iter().map(read_id).collect::<Result<_, _>>()?))
                } else {
                    if a.get("objects").is_some() { return Err("optional objects: decline has no objects".into()); }
                    Answer::OptionalObjects(None)
                }
            },
            Some(a) if a.get("objects").is_some() => Answer::Objects(list(a, "objects").iter().map(read_id).collect::<Result<_, _>>()?),
            Some(a) if a.get("player").is_some() => Answer::Player(a["player"].as_u64().ok_or("player: a player's number")? as usize),
            Some(a) if a.get("call").is_some() => Answer::CoinCall(match a["call"].as_str() {
                Some("heads") => game::start::CoinFace::Heads,
                Some("tails") => game::start::CoinFace::Tails,
                _ => return Err("a coin call is heads or tails".into()),
            }),
            Some(a) if a.get("alternative").is_some() => Answer::Alternative(a["alternative"].as_u64().ok_or("alternative: an offered action index")? as usize),
            Some(a) if a.get("skip").is_some() => Answer::Skip(game::turn::SkipId(a["skip"].as_u64().and_then(|n| u32::try_from(n).ok()).ok_or("skip: an offered skip effect's number")?)),
            Some(a) if a.get("library_position").is_some() => Answer::LibraryPosition(read_library_position(&a["library_position"])?),
            Some(a) if a.get("pile").is_some() => Answer::Pile(a["pile"].as_u64().ok_or("pile: the place of a pile")? as usize),
            Some(a) if a.get("prevention").is_some()=>Answer::Prevention(a["prevention"].as_array().ok_or("prevention: an array of hit/amount pairs")?.iter().map(|pair| {
                let pair=pair.as_array().filter(|p|p.len()==2).ok_or("prevention: a hit and integer amount")?;
                let hit=usize::try_from(pair[0].as_u64().ok_or("prevention: nonnegative hit id")?).map_err(|_|"prevention: hit overflow")?;
                let amount=u32::try_from(pair[1].as_u64().ok_or("prevention: nonnegative integer amount")?).map_err(|_|"prevention: amount overflow")?;
                Ok((hit,amount))
            }).collect::<Result<_,String>>()?),
            Some(a) if a.get("holders").is_some() => Answer::Holders(a["holders"].as_array().ok_or("holders: an array of offered players/permanents")?.iter().map(read_target).collect::<Result<_, _>>()?),
            Some(a) if a.get("attach").is_some() => Answer::Attach(read_target(&a["attach"])?),
            Some(a) if a.get("defender").is_some() => Answer::Defender(read_target(&a["defender"])?),
            Some(a) if a.get("mana").is_some() => Answer::Mana(read_mana(&a["mana"])?),
            Some(a) if a.get("value").is_some() => Answer::Value(read_bound_value(&a["value"])?),
            Some(a) if a.get("replacement").is_some() => {
                let said = &a["replacement"];
                let identity = if said.get("rule").is_some() {
                    offered.iter().find(|effect| replacement(effect) == *said)
                        .ok_or_else(|| Error::unreferable("that replacement identity was not offered"))?.clone()
                } else {
                    let key = if let Some(index) = said["ability"].as_u64() {
                        game::binding::AbilityKey::printed(index as usize)
                    } else {
                        offered.iter().find(|e| oid(e.source) == said["source"]
                            && json!(e.part()) == said["part"]
                            && e.ability().is_some_and(|key| replacement_ability(key.clone()) == said["ability"]))
                            .and_then(|e| e.ability()).ok_or_else(|| Error::unreferable("that replacement identity was not offered"))?.clone()
                    };
                    game::operation::ReplacementId::from_ability(read_id(&said["source"])?, key,
                        said["part"].as_u64().ok_or("a replacement effect needs its part index")? as usize)
                };
                Answer::Replacement(identity)
            },
            Some(a) if a.get("creature_type").is_some() => {
                let name = a["creature_type"].as_str().ok_or("a creature type is named by a string")?;
                Answer::Value(game::binding::Value::CreatureType(ir::Subtype::from_name(name).ok_or_else(|| format!("{name} is no subtype"))?))
            }
            Some(a) if a.get("top").is_some() || a.get("away").is_some() => Answer::Arrange {
                top: list(a, "top").iter().map(read_id).collect::<Result<_, _>>()?,
                away: list(a, "away").iter().map(read_id).collect::<Result<_, _>>()?,
            },
            other => {
                return Err(format!(
                    "an answer is \"yes\", \"no\", {{\"objects\":[…]}}, {{\"mana\":[…]}}, {{\"creature_type\":\"…\"}}, {{\"replacement\":{{…}}}} or {{\"top\":[…],\"away\":[…]}}, not {other:?}"
                ).into());
            }
        }),
        other => return Err(format!("unknown move \"{other}\"").into()),
    })
}

/// A game with these cards where the request says, at the untap step of the first player's
/// first turn. Every card must be one the engine runs.
/// Reuse the last compiled card set across independent games. Keeping one entry bounds
/// adapter retention; games already made retain their immutable catalog by Arc.
struct Catalog<'a> {
    faces: BTreeMap<&'a str, Vec<&'a corpus::Face>>,
    compiled: std::sync::Mutex<Option<(Vec<String>, std::sync::Arc<Vec<CardDef>>) >>,
}
impl<'a> Catalog<'a> {
    fn new(faces: BTreeMap<&'a str, Vec<&'a corpus::Face>>) -> Self {
        Self { faces, compiled: Default::default() }
    }
    fn definitions(&self, names: &[String]) -> Result<std::sync::Arc<Vec<CardDef>>, Error> {
        let mut compiled = self.compiled.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached, defs)) = compiled.as_ref() {
            if cached == names { return Ok(defs.clone()); }
        }
        #[cfg(feature = "profile")]
        let _compile = game::profile::span(game::profile::Scope::CompileCatalog);
        let defs = names.iter().map(|name| {
            let face = self.faces.get(name.as_str()).ok_or_else(|| Error::unreferable(format!("unknown card: {name}")))?;
            let def = CardDef::from_faces(face).map_err(|e| Error::unsupported(format!("the grammar does not read {name}: {e}")))?;
            check(&def).map_err(|u| Error::unsupported(format!("the engine does not run {name}: {}", u.0)))?;
            Ok(def)
        }).collect::<Result<Vec<_>, Error>>()?;
        let defs = std::sync::Arc::new(defs);
        *compiled = Some((names.to_vec(), defs.clone()));
        Ok(defs)
    }
}

/// Adapter construction through the same neutral state vocabulary. Derived Oracle
/// observations are checked by the adapter, never installed as characteristics.
/// A position that does not say what construction takes is malformed; one the game's bounded
/// construction refuses (`Game::prepare_initial_position` and its siblings) is unsupported.
fn new_position(faces: &Catalog, position: &Value) -> Result<Game, Error> {
    fn fields(value: &Value, required: &[&str]) -> Result<(), String> {
        let object=value.as_object().ok_or("initial position requires an object")?;
        if object.len()!=required.len() || required.iter().any(|key|!object.contains_key(*key)) {
            return Err("unknown/missing initial construction fields".into());
        }
        Ok(())
    }
    fn player(value: &Value) -> Result<usize,String> {
        match value.as_str() {Some("A")=>Ok(0),Some("B")=>Ok(1),_=>Err("unknown initial player".into())}
    }
    fields(position,&["timing","next","players","zones","objects","history","causal"])?;
    fields(&position["timing"],&["turn","step","active"])?;
    fields(&position["next"],&["kind","chooser"])?;
    if position["next"]["kind"]!="priority" {return Err(Error::unsupported("initial position requires priority"));}
    let turn=u32::try_from(position["timing"]["turn"].as_u64().ok_or("initial turn must be positive")?).map_err(|_|"initial turn overflow")?;
    let part=match position["timing"]["step"].as_str() {
        Some("upkeep")=>ir::Part::Upkeep,Some("draw")=>ir::Part::Draw,
        Some("precombat_main")=>ir::Part::PrecombatMain,Some("postcombat_main")=>ir::Part::PostcombatMain,
        Some("begin_combat")=>ir::Part::BeginningOfCombat,Some("end_combat")=>ir::Part::EndOfCombat,Some("end")=>ir::Part::End,
        _=>return Err(Error::unsupported("unsupported initial phase")),
    };
    let active=player(&position["timing"]["active"])?;
    let priority=player(&position["next"]["chooser"])?;
    fields(&position["players"],&["A","B"])?;
    fields(&position["history"],&["turn_structure","pending","effects","turn_events","previous_turn_spells","land_plays_this_turn","ability_uses","consecutive_passes","control_since"])?;
    let history=&position["history"];
    if history["turn_structure"]!="ordinary_A_first" || !matches!(history["pending"].as_str(),Some("none"|"represented_delayed")) || !matches!(history["effects"].as_str(),Some("intrinsic_only"|"represented_only"))
        || !matches!(history["turn_events"].as_str(),Some("empty"|"represented_casts_only")) || history["previous_turn_spells"]!=0
        || !matches!(history["ability_uses"].as_str(),Some("none"|"represented_activations_only")) {
        return Err(Error::unsupported("unsupported initial history"));
    }
    fields(&history["land_plays_this_turn"],&["A","B"])?;
    let land_plays=["A","B"].map(|player|history["land_plays_this_turn"][player].as_u64()
        .filter(|n|*n<=1).map(|n|n as usize).ok_or(Error::unsupported("unsupported initial land-play consumption")));
    let land_plays=[land_plays[0].clone()?,land_plays[1].clone()?];
    let passes=history["consecutive_passes"].as_u64().ok_or("missing initial consecutive passes")? as usize;
    let objects=position["objects"].as_object().ok_or("missing initial objects")?;
    let mut names=Vec::new(); let mut definitions=BTreeMap::new();
    for object in objects.values() {
        fields(object,&["name","owner","controller","tapped","counters","damage"])?;
        let name=object["name"].as_str().ok_or("initial definition requires a name")?;
        if !definitions.contains_key(name) {definitions.insert(name.to_owned(),names.len());names.push(name.to_owned());}
    }
    let mut g=Game::new(faces.definitions(&names)?,2,20);
    for (index,seat) in ["A","B"].iter().enumerate() {
        fields(&position["players"][seat],&["life","mana"])?;
        let life=position["players"][seat]["life"].as_i64().and_then(|n|i32::try_from(n).ok()).filter(|n|*n>0).ok_or("initial life must be positive")?;
        g.players[index].life=life;
        let mana=position["players"][seat]["mana"].as_object().ok_or("initial mana must be an object")?;
        let mut total=0u32;
        for (kind,count) in mana {
            let count=count.as_u64().and_then(|n|u32::try_from(n).ok()).filter(|n|*n>0).ok_or("invalid initial mana amount")?;
            total=total.checked_add(count).ok_or("initial mana overflow")?;
            if kind=="C" {g.players[index].pool.colorless=count;}
            else if let Some(color)=["W","U","B","R","G"].iter().position(|color|*color==kind) {g.players[index].pool.colors[color]=count;}
            else {return Err(Error::unsupported("only plain initial WUBRGC mana is supported"));}
        }
    }
    let zones=[("A.library",Zone::Library), ("B.library",Zone::Library),
        ("A.hand",Zone::Hand),("B.hand",Zone::Hand),("A.graveyard",Zone::Graveyard),("B.graveyard",Zone::Graveyard),
        ("battlefield",Zone::Battlefield),("stack",Zone::Stack),("exile",Zone::Exile),("command",Zone::Command)];
    fields(&position["zones"],&zones.iter().map(|(key,_)|*key).collect::<Vec<_>>())?;
    let mut placed=std::collections::BTreeSet::new();let mut controls=Vec::new();let mut handles=BTreeMap::new();
    for (name,zone) in zones {
        let refs=position["zones"][name].as_array().ok_or("initial zone must be an array")?;
        if matches!(zone,Zone::Stack|Zone::Command) && !refs.is_empty() {return Err(Error::unsupported("unsupported initial stack/command"));}
        let ordered:Vec<_>=if zone==Zone::Library {refs.iter().rev().collect()}else{refs.iter().collect()};
        for reference in ordered {
            let reference=reference.as_str().ok_or("initial reference must be a string")?;
            if !placed.insert(reference) {return Err("duplicate initial object placement".into());}
            let object=objects.get(reference).ok_or("unknown placed initial object")?;
            let owner=player(&object["owner"])?; let controller=player(&object["controller"])?;
            if (name.starts_with("A.") && owner!=0)||(name.starts_with("B.") && owner!=1)
                ||(zone!=Zone::Battlefield && owner!=controller) {return Err("initial ownership/zone mismatch".into());}
            let tapped=object["tapped"].as_bool().ok_or("missing initial tapped status")?;
            if zone!=Zone::Battlefield && tapped {return Err("nonpermanent cannot start tapped".into());}
            let id=g.put(owner,definitions[object["name"].as_str().unwrap()],zone);
            handles.insert(reference,id);
            let counters=object["counters"].as_object().ok_or("initial counters must be an object")?;
            let damage=object["damage"].as_i64().and_then(|n|i32::try_from(n).ok()).filter(|n|*n>=0).ok_or("invalid initial damage")?;
            if zone!=Zone::Battlefield && (!counters.is_empty() || damage!=0) {return Err("initial counters/damage require battlefield".into());}
            for (kind,count) in counters {
                game::layers::counter_ok(kind).map_err(|e|e.0)?;
                let count=count.as_u64().and_then(|n|u32::try_from(n).ok()).filter(|n|*n>0).ok_or("invalid initial counter count")?;
                g.set_initial_counters(id,kind,count);
            }
            if zone==Zone::Battlefield {g.set_initial_damage(id,damage)?;}
            if zone==Zone::Battlefield {
                g.set_tapped(id,tapped);
                let this_turn=match history["control_since"][reference].as_str() {
                    Some("before_turn")=>false,Some("this_turn")=>true,_=>return Err("missing initial control duration".into()),
                };
                let base=position["causal"]["base_controllers"].get(reference).map(player).transpose()?.unwrap_or(controller);
                controls.push((id,base,this_turn));
            }
        }
    }
    if placed.len()!=objects.len() || history["control_since"].as_object().ok_or("missing control duration map")?.len()!=controls.len() {
        return Err("initial membership/control duration differs".into());
    }
    // Establish the declared controller/control-duration view before validating
    // residual effects and their recipients. g.put initially uses physical owner.
    // Final settling below still checks the fully installed premise.
    g.prepare_initial_position(turn,part,active,priority,passes,&controls).map_err(Error::unsupported)?;
    let mut causal_fields=vec!["linked_exile","prevention","continuous_effects","base_controllers"];
    if position["causal"].get("activation_uses").is_some() {causal_fields.push("activation_uses");}
    if position["causal"].get("source_objects").is_some() {causal_fields.push("source_objects");}
    if position["causal"].get("delayed_triggers").is_some() {causal_fields.push("delayed_triggers");}
    if position["causal"].get("casts_this_turn").is_some() {causal_fields.push("casts_this_turn");}
    fields(&position["causal"],&causal_fields)?;
    if let Some(sources)=position["causal"].get("source_objects") {
        for (label,source) in sources.as_object().ok_or("initial source objects must be a map")? {
            fields(source,&["physical","name"])?;
            if handles.contains_key(label.as_str()) {return Err("initial source label collides with a live object".into());}
            let physical=*handles.get(source["physical"].as_str().ok_or("missing historical physical card")?).ok_or("unknown historical physical card")?;
            if source["name"]!=g.audit_game_objects()["objects"][physical.0.to_string()]["definition"] {
                return Err("initial historical source definition differs from its physical printing".into());
            }
            let source=g.initial_continuous_source(physical);
            handles.insert(label.as_str(),source);
        }
    }
    for link in position["causal"]["linked_exile"].as_array().ok_or("linked exile must be a list")? {
        fields(link,&["source","ability","exiled"])?;
        fields(&link["ability"],&["source","definition","paragraph","origin"])?;
        if link["ability"]["source"]!=link["source"] || link["ability"]["origin"]!=json!({"kind":"printed"}) {
            return Err("initial linked exile requires an own printed producer".into());
        }
        let source=*handles.get(link["source"].as_str().ok_or("missing linked source")?).ok_or("unknown linked source")?;
        let exiled=*handles.get(link["exiled"].as_str().ok_or("missing linked exile")?).ok_or("unknown linked exile")?;
        let paragraph=link["ability"]["paragraph"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid linked paragraph")?;
        g.set_initial_linked_exile(source,paragraph,exiled).map_err(Error::unsupported)?;
    }
    let continuous=position["causal"]["continuous_effects"].as_array().ok_or("continuous effects must be a list")?;
    let bases=position["causal"]["base_controllers"].as_object().ok_or("missing continuous base controllers")?;
    let mut subject_refs=std::collections::BTreeSet::new();
    let mut continuous_ids=std::collections::BTreeSet::new();
    for effect in continuous {
        fields(effect,&["id","producer","controller","subjects","changes","expires"])?;
        let identity=effect["id"].as_str().filter(|id|!id.is_empty()).ok_or("missing continuous occurrence id")?;
        if !continuous_ids.insert(identity) {return Err("duplicate continuous occurrence id".into());}
        fields(&effect["producer"],&["source","definition","paragraph","origin"])?;
        if effect["producer"]["origin"]!=json!({"kind":"printed"}) {return Err("initial continuous producer must be printed".into());}
        let reference=effect["producer"]["source"].as_str().ok_or("missing continuous producer source")?;
        if objects.contains_key(reference) {return Err("continuous producer must be a departed spell incarnation".into());}
        let source=if let Some(source)=handles.get(reference) {*source} else {
            // The only historical declaration admitted here is the departed stack
            // incarnation immediately preceding this physical spell in graveyard.
            let (physical,generation)=reference.rsplit_once('@').ok_or("invalid departed producer reference")?;
            let generation=generation.parse::<u32>().ok().filter(|n|*n>0).ok_or("invalid departed producer generation")?;
            if reference!=format!("{physical}@{generation}") {return Err("noncanonical departed producer reference".into());}
            let next=generation.checked_add(1).ok_or("departed producer generation overflow")?;
            let current=format!("{physical}@{next}");
            if !["A.graveyard","B.graveyard"].iter().any(|zone|position["zones"][zone].as_array().unwrap().contains(&json!(current))) {
                return Err("departed continuous producer needs its declared graveyard successor".into());
            }
            let live=*handles.get(current.as_str()).ok_or("missing departed producer successor")?;
            let source=g.initial_continuous_source(live);
            handles.insert(reference,source);source
        };
        let paragraph=effect["producer"]["paragraph"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid continuous paragraph")?;
        let subjects=effect["subjects"].as_array().ok_or("missing continuous subjects")?.iter().map(|subject| {
            let subject=subject.as_str().ok_or("invalid continuous subject")?;
            subject_refs.insert(subject);
            handles.get(subject).copied().ok_or("unknown continuous subject")
        }).collect::<Result<Vec<_>,_>>()?;
        g.set_initial_continuous(source,paragraph,player(&effect["controller"])?,subjects,&effect["changes"],&effect["expires"]).map_err(Error::unsupported)?;
    }
    g.verify_initial_continuous_inventory().map_err(Error::unsupported)?;
    if subject_refs!=bases.keys().map(String::as_str).collect() {return Err("continuous base controllers must cover exactly affected subjects".into());}
    let prevention=position["causal"]["prevention"].as_array().ok_or("prevention must be a list")?;
    if (history["effects"]=="represented_only")!=(!prevention.is_empty() || !continuous.is_empty()) {return Err("initial prevention/effects declaration differs".into());}
    let mut effect_ids=std::collections::BTreeSet::new();
    for effect in prevention {
        fields(effect,&["id","producer","controller","recipients","damage","remaining","expires"])?;
        let effect_id=effect["id"].as_str().filter(|id|!id.is_empty()).ok_or("missing prevention occurrence id")?;
        if !effect_ids.insert(effect_id) {return Err("duplicate prevention occurrence id".into());}
        fields(&effect["producer"],&["source","definition","paragraph","origin"])?;
        if effect["producer"]["origin"]!=json!({"kind":"printed"}) || effect["expires"]!=json!({"kind":"end_of_turn","turn":turn}) {
            return Err(Error::unsupported("unsupported initial prevention producer/lifetime"));
        }
        let source=*handles.get(effect["producer"]["source"].as_str().ok_or("missing prevention source")?).ok_or("unknown prevention source")?;
        let paragraph=effect["producer"]["paragraph"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid prevention paragraph")?;
        let recipients=effect["recipients"].as_array().filter(|to|to.len()==1).ok_or("initial prevention requires one recipient")?;
        let to=&recipients[0];
        let recipient=if to.get("player").is_some() {
            fields(to,&["player"])?;Target::Player(player(&to["player"])?)
        } else {
            fields(to,&["object"])?;Target::Object(*handles.get(to["object"].as_str().ok_or("missing prevention recipient")?).ok_or("unknown prevention recipient")?)
        };
        let remaining=effect["remaining"].as_u64().and_then(|n|u32::try_from(n).ok()).ok_or("invalid prevention remaining capacity")?;
        let kind=g.set_initial_prevention(source,paragraph,player(&effect["controller"])?,recipient,remaining).map_err(Error::unsupported)?;
        let kind=match kind {ir::event::DamageKind::Any=>"any",ir::event::DamageKind::Combat=>"combat",ir::event::DamageKind::Noncombat=>"noncombat"};
        if effect["damage"]!=kind {return Err("initial prevention damage kind differs from producer".into());}
    }
    let empty_delays=Vec::new();
    let delays=position["causal"].get("delayed_triggers").map(|value|value.as_array().ok_or("initial delayed inventory must be a list")).transpose()?.unwrap_or(&empty_delays);
    if (history["pending"]=="represented_delayed")!=!delays.is_empty() {return Err("initial delayed inventory differs from declared history".into());}
    let mut delayed_ids=std::collections::BTreeSet::new();
    for delayed in delays {
        fields(delayed,&["id","producer","controller","clause","retained","when","once","expires"])?;
        if !delayed_ids.insert(delayed["id"].as_str().ok_or("missing delayed occurrence id")?) {return Err("duplicate delayed occurrence id".into());}
        fields(&delayed["producer"],&["source","definition","paragraph","origin"])?;
        if delayed["producer"]["origin"]!=json!({"kind":"printed"}) {return Err("initial delayed producer must be printed".into());}
        let source=*handles.get(delayed["producer"]["source"].as_str().ok_or("missing delayed source")?).ok_or("unknown delayed source")?;
        let paragraph=delayed["producer"]["paragraph"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid delayed paragraph")?;
        let clause=delayed["clause"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid delayed clause")?;
        let retained=delayed["retained"].as_array().filter(|values|values.len()==1).ok_or("initial delayed capture requires one exile result")?;
        fields(&retained[0],&["kind","objects"])?;
        if retained[0]["kind"]!="exiled" {return Err(Error::unsupported("unsupported initial delayed retained role"));}
        let exiled=retained[0]["objects"].as_array().ok_or("initial exiled result must be a list")?.iter().map(|reference|
            handles.get(reference.as_str().ok_or("invalid retained exile object")?).copied().ok_or("unknown retained exile object")
        ).collect::<Result<Vec<_>,_>>()?;
        g.set_initial_delayed_exile(source,paragraph,player(&delayed["controller"])?,clause,&exiled).map_err(Error::unsupported)?;
        let observed=g.audit_game_objects()["delayed_triggers"].as_array().unwrap().last().unwrap().clone();
        for field in ["when","once","expires"] {if delayed[field]!=observed[field] {return Err(format!("initial delayed {field} differs from its Oracle producer").into());}}
    }
    g.verify_initial_delayed_inventory().map_err(Error::unsupported)?;
    g.set_initial_land_plays(land_plays).map_err(Error::unsupported)?;
    let empty_casts=Vec::new();
    let casts=position["causal"].get("casts_this_turn").map(|value|value.as_array().ok_or("initial cast inventory must be a list")).transpose()?.unwrap_or(&empty_casts);
    if (history["turn_events"]=="represented_casts_only")!=!casts.is_empty() {return Err("initial cast inventory differs from declared history".into());}
    let casts=casts.iter().map(|cast| {
        fields(cast,&["source","player"])?;
        let source=*handles.get(cast["source"].as_str().ok_or("missing initial cast source")?).ok_or("unknown initial cast source")?;
        Ok((source,player(&cast["player"])?))
    }).collect::<Result<Vec<_>,String>>()?;
    g.set_initial_cast_history(&casts).map_err(Error::unsupported)?;
    // An explicit no-use premise already declares the empty inventory. Ordinary
    // observations without this field still say nothing about prior uses.
    let empty_uses=Vec::new();
    let uses=match position["causal"].get("activation_uses") {
        Some(uses)=>uses.as_array().ok_or("initial activation uses must be a complete list")?,
        None if history["ability_uses"]=="none"=>&empty_uses,
        None=>return Err("represented activation history requires its complete inventory".into()),
    };
    if (history["ability_uses"]=="represented_activations_only")!=!uses.is_empty() {
        return Err("initial activation inventory disagrees with declared usage history".into());
    }
    let uses=uses.iter().map(|row| {
        use game::audit::{ActivationUseLifetime,InitialActivationUse};
        fields(row,&["ability","lifetime","count"])?;
        let ability=&row["ability"];
        fields(ability,&["source","definition","paragraph","origin"])?;
        if ability["origin"]!=json!({"kind":"printed"}) {return Err("initial activation use must name an own printed ability".into());}
        let reference=ability["source"].as_str().ok_or("missing activation source")?;
        let source=*handles.get(reference).ok_or("unknown activation source")?;
        let name=objects.get(reference).and_then(|object|object["name"].as_str()).ok_or("activation source must be a declared current object")?;
        let definition=faces.faces.get(name).and_then(|faces|faces.first()).ok_or("activation definition unavailable")?;
        if ability["definition"]!=json!({"oracle":definition.oracle_id}) {
            return Err("initial activation definition differs from its source".into());
        }
        let paragraph=ability["paragraph"].as_u64().and_then(|n|usize::try_from(n).ok()).ok_or("invalid activation paragraph")?;
        let lifetime=match row["lifetime"]["kind"].as_str() {
            Some("turn")=>{
                fields(&row["lifetime"],&["kind","turn"])?;
                if row["lifetime"]["turn"]!=turn {return Err("initial activation belongs to a different turn".into());}
                ActivationUseLifetime::Turn
            },
            Some("incarnation")=>{fields(&row["lifetime"],&["kind"])?;ActivationUseLifetime::Incarnation},
            _=>return Err(Error::unsupported("unsupported initial activation lifetime")),
        };
        let count=row["count"].as_u64().and_then(|n|u32::try_from(n).ok()).filter(|n|*n>0).ok_or("initial activation count must be positive")?;
        Ok(InitialActivationUse{source,paragraph,lifetime,count})
    }).collect::<Result<Vec<_>,Error>>()?;
    g.set_initial_activation_uses(&uses).map_err(Error::unsupported)?;
    g.finish_initial_position().map_err(Error::unsupported)?;
    for (reference,id) in &handles {
        if let Some(object)=objects.get(*reference) {
            if g.controller(*id)!=player(&object["controller"])? {return Err("initial effective controller differs".into());}
        }
    }
    Ok(g)
}

/// The life total each player begins with (103.4): 20 unless the setup says. One the engine
/// cannot hold is refused as an engine representation limit, never truncated.
fn starting_life(request: &Value) -> Result<i32, String> {
    request.get("life").map_or(Ok(20), |life| life.as_i64().and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("starting life is a whole number in the engine range {} to {}", i32::MIN, i32::MAX)))
}

fn new_game(faces: &Catalog, request: &Value) -> Result<Game, Error> {
    if let Some(position)=request.get("position") {
        // a position takes nothing else but the setup flag
        if request.as_object().is_none_or(|fields|fields.keys().any(|key|!["cmd","position","ask_forced"].contains(&key.as_str()))) {return Err("unknown initial request fields".into());}
        let mut g = new_position(faces,position)?;
        g.set_ask_forced(ask_forced(request)?);
        return Ok(g);
    }

    const ZONES: [(&str, Zone, bool); 6] = [
        ("library", Zone::Library, false),
        ("hand", Zone::Hand, false),
        ("battlefield", Zone::Battlefield, false),
        ("battlefield_tapped", Zone::Battlefield, true),
        ("graveyard", Zone::Graveyard, false),
        ("exile", Zone::Exile, false),
    ];
    let seats = list(request, "players");
    if seats.len() < 2 {
        return Err("a game has at least two players".into());
    }
    let mut names_used: Vec<String> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut puts = Vec::new();
    for (player, seat) in seats.iter().enumerate() {
        for (field, zone, tapped) in ZONES {
            for name in list(seat, field) {
                let name = name.as_str().ok_or("a card is named by a string")?;
                if !index.contains_key(name) {
                    index.insert(name.to_string(), names_used.len());
                    names_used.push(name.to_string());
                }
                puts.push((player, index[name], zone, tapped));
            }
        }
    }
    let life = starting_life(request)?;
    let defs = faces.definitions(&names_used)?;
    #[cfg(feature = "profile")]
    let _setup = game::profile::span(game::profile::Scope::SetupGame);
    let mut g = Game::new(defs, seats.len(), life);
    g.set_ask_forced(ask_forced(request)?);
    for (player, def, zone, tapped) in puts {
        let o = g.put(player, def, zone);
        g.set_tapped(o, tapped);
        // a permanent of the board is as it would have entered ("This artifact enters tapped.")
        if zone == Zone::Battlefield {
            g.try_as_entered(o).map_err(|e|Error::unsupported(e.0))?;
        }
    }
    g.try_begin()?;
    Ok(g)
}

/// A game from its start (103): each deck shuffled, hands of seven drawn, the players asked
/// about mulligans. A deck is a list of card names, one for each copy, or the path of a deck
/// file whose main deck it is. Every card must be one the engine runs.
fn start_game(faces: &Catalog, request: &Value) -> Result<Game, Error> {
    let commander_format = match request.get("format").and_then(Value::as_str) {
        None | Some("constructed") => false,
        Some("commander") => true,
        Some(other) => return Err(Error::unsupported(format!("unsupported format {other}"))),
    };
    if commander_format && request.get("sideboards").is_some() {
        return Err(Error::illegal("Commander games do not use sideboards (903.5e)"));
    }
    let decks = list(request, "decks");
    if decks.len() < 2 {
        return Err("a game has at least two players".into());
    }
    let mut names_used: Vec<String> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut cards = Vec::new();
    for deck in decks {
        let names: Vec<String> = match deck {
            Value::String(path) => {
                let text = std::fs::read_to_string(path).map_err(|e| Error::unreferable(format!("cannot read {path}: {e}")))?;
                if commander_format && text.lines().any(|line| line.trim().starts_with("SB:")) {
                    return Err(Error::illegal("Commander games do not use sideboards (903.5e)"));
                }
                game::start::deck_list(&text)
            },
            Value::Array(names) => names.iter().map(|n| n.as_str().map(str::to_string).ok_or("a card is named by a string")).collect::<Result<_, _>>()?,
            _ => return Err("a deck is a list of card names or the path of a deck file".into()),
        };
        let mut deck = Vec::new();
        for name in names {
            if !index.contains_key(&name) {
                index.insert(name.clone(), names_used.len());
                names_used.push(name.clone());
            }
            deck.push(index[&name]);
        }
        cards.push(deck);
    }
    let designated = match request.get("companions") {
        None => vec![Value::Null;cards.len()],
        Some(Value::Array(v)) if v.len()==cards.len()=>v.clone(),
        _=>return Err("companions names one card or null per player".into()),
    };
    let mut companions=Vec::new();
    for named in designated {
        if named.is_null(){companions.push(None);continue;}
        let name=named.as_str().ok_or("a companion is one card name or null")?.to_owned();
        if !index.contains_key(&name){index.insert(name.clone(),names_used.len());names_used.push(name.clone());}
        companions.push(Some(index[&name]));
    }
    let mut commanders = Vec::new();
    if commander_format {
        if companions.iter().any(Option::is_some) { return Err(Error::unsupported("Commander companion setup is not implemented")); }
        let names = request.get("commanders").and_then(Value::as_array).filter(|names| names.len() == cards.len())
            .ok_or("Commander setup names one commander per player")?;
        for name in names {
            let name = name.as_str().ok_or("one commander card name per player")?.to_owned();
            if !index.contains_key(&name) { index.insert(name.clone(), names_used.len()); names_used.push(name.clone()); }
            commanders.push(index[&name]);
        }
        if request.get("life").is_some_and(|life| life.as_i64() != Some(40)) {
            return Err(Error::illegal("Commander starting life is 40 (903.7)"));
        }
    } else if request.get("commanders").is_some() {
        return Err("designated commanders require format: commander".into());
    }
    let number = |field: &str, or: u64| request.get(field).and_then(Value::as_u64).unwrap_or(or);
    let first = number("first", 0) as usize;
    if first >= cards.len() {
        return Err(Error::unreferable("the starting player is one of the players"));
    }
    let life = starting_life(request)?;
    let defs = faces.definitions(&names_used)?;
    #[cfg(feature = "profile")]
    let _setup = game::profile::span(game::profile::Scope::SetupGame);
    // the orders the initial shuffles produced in a game played elsewhere (103.3)
    let orders: Option<Vec<Vec<usize>>> = match request.get("orders") {
        None => None,
        Some(_) if request.get("seed").is_some() => return Err("orders and seed are exclusive: with orders no random number is drawn".into()),
        Some(Value::Array(orders)) => Some(orders.iter().map(|order| order.as_array()
            .and_then(|order| order.iter().map(|i| i.as_u64().map(|i| i as usize)).collect::<Option<Vec<_>>>())
            .ok_or("an order is a list of positions in that player's deck")).collect::<Result<_, _>>()?),
        Some(_) => return Err("orders gives one library order per player".into()),
    };
    let initial = match &orders { Some(orders) => game::start::Initial::Orders(orders), None => game::start::Initial::Seed(number("seed", 0)) };
    let ask = ask_forced(request)?;
    let mut g = if commander_format { Game::start_commander_from(defs, &cards, &commanders, first, initial) }
        else { Game::start_from(defs, &cards, &companions, life, first, initial) }?;
    g.set_ask_forced(ask);
    Ok(g)
}

/// `"ask_forced":true` (setup): a question the rules have a player answer is asked even when
/// it has one possible answer, as a record of a game played elsewhere has it (508.4).
fn ask_forced(request: &Value) -> Result<bool, String> {
    match request.get("ask_forced") {
        None => Ok(false),
        Some(Value::Bool(ask)) => Ok(*ask),
        Some(_) => Err("ask_forced is true or false".into()),
    }
}

/// A request saying a field its command does not take is malformed, refused before anything
/// else it says is read: a misspelled `viewer` would otherwise be acted on and answered with
/// the whole game.
fn request_fields(input: &Value, fields: &[&str]) -> Result<(), Error> {
    let cmd = input["cmd"].as_str().unwrap_or("");
    match input.as_object().into_iter().flatten().map(|(k, _)| k.as_str()).find(|k| *k != "cmd" && !fields.contains(k)) {
        Some(unknown) => Err(format!("{cmd} has no \"{unknown}\"; it says {}", fields.join(", ")).into()),
        None => Ok(()),
    }
}

/// A request's optional flag: absent is false, and anything but true or false is malformed.
fn flag(input: &Value, field: &str) -> Result<bool, Error> {
    match input.get(field) {
        None => Ok(false),
        Some(Value::Bool(set)) => Ok(*set),
        Some(other) => Err(format!("{field} is true or false, not {other}").into()),
    }
}

/// Publish a new session only after setup and its complete first response succeed. The old
/// session is a different simulation, not a rollback target for a failed new one.
fn reset(
    game: &mut Option<Simulation>,
    session_id: &mut u64,
    logged: &mut Delivery,
    input: &Value,
    setup: impl FnOnce() -> Result<Game, Error>,
) -> Result<Value, Error> {
    let next = session_id.checked_add(1).ok_or_else(|| Error::unsupported("session ID exhausted"))?;
    let setup_fault = || Error::Fault("simulation setup fault".into());
    let mut prepared = Simulation::from_setup(setup).map_err(|error| match error {
        SetupError::Rejected(error) => error,
        // As for runtime faults, panic text may contain hidden information. The default
        // hook retains diagnostics on stderr without exporting them to a seat.
        SetupError::Fault { .. } => setup_fault(),
    })?;
    prepared.request().map_err(|_| setup_fault())?;
    let mut delivery = Delivery::default();
    let response = reply_state(&mut prepared, next, &mut delivery, input).map_err(|error| {
        // The failed candidate is discarded, so the caller cannot inspect its fault.
        if prepared.fault().is_some() { setup_fault() } else { error }
    })?;
    *game = Some(prepared);
    *session_id = next;
    *logged = delivery;
    Ok(response)
}

/// The wire this pipe speaks to a trusted harness: `new`/`start`/`act`/`state`/
/// `version`/`library_order`/`set_library_order`,
/// with numeric object and player ids.
const HARNESS_API: &str = "harness/1";

/// Which engine this is (`"cmd":"version"`): what a test or report names to say what it ran.
fn version() -> Value {
    let source = match (option_env!("GAME_SOURCE_REVISION"), option_env!("GAME_SOURCE_DIRTY")) {
        (Some(revision), dirty) if !revision.is_empty() => json!({ "revision": revision, "dirty": dirty.map(|d| d == "true") }),
        _ => Value::Null,
    };
    json!({ "package": env!("CARGO_PKG_VERSION"), "source": source,
            "apis": { "harness": HARNESS_API, "player": game::prompt::API },
            "random": game::start::RANDOM })
}

struct PaymentProbe {
    proof: Value,
    pools: Vec<Pool>,
    outcome: Result<(), game::play::ActionError>,
}

/// Execute only an isolated clone. The live RNG, pending request, labels and
/// delivery buffers are untouched. A missing completed receipt is evidence of
/// an interrupted announcement, never a guessed zero cost.
fn payment_probe(g:&Game, player:usize, request:&Value) -> Result<PaymentProbe,Error> {
    let pool=&g.players.get(g.decision_actor()).ok_or_else(|| Error::unreferable("no such player"))?.pool;
    let offered=match &g.decision {
        Some(game::state::Decision::Choose {choice:game::state::Choice::Replacement{among,..},..})=>among.as_slice(),
        _=>&[],
    };
    optional_named(request,g)?;
    let mv=read_payment_move(g,player,request,pool,offered)?;
    Ok(payment_probe_move(g, player, request, mv))
}

fn payment_probe_move(g:&Game, player:usize, request:&Value, mv:Move) -> PaymentProbe {
    let mut quoted=g.clone();
    quoted.begin_payment_audit();
    let since=quoted.log.end();
    let outcome=quoted.attempt(player,mv);
    let payments=quoted.payment_audit();
    let events=quoted.log.since(since).iter().map(|e|event(&quoted,None,e)).collect::<Vec<_>>();
    let reached_main=match request["kind"].as_str() {
        Some("cast")=>payments.last().is_some_and(|p|p["kind"]=="spell")
            && events.iter().any(|e|e["event"]=="cast"),
        Some("activate")=>{
            let activations=events.iter().filter(|e|e["event"]=="activated").collect::<Vec<_>>();
            payments.last().is_some_and(|p|p["kind"]=="ability" && p["source"]==request["source"] && p["path"]==json!([]))
                && activations.last().is_some_and(|e|e["source"]==request["source"])
        }
        _=>true,
    };
    let complete=outcome.is_ok() && reached_main && payments.last().is_some_and(|p|p["path"]==json!([]) && !p["left"].is_null());
    let suspended = if outcome.is_ok() && !complete && payments.last().is_some_and(|p|p["path"]==json!([]) && !p["left"].is_null()) {
        quoted.payment_order_boundary().filter(|boundary| payments.last().is_some_and(|p|
            p["source"]==boundary["source"] && p["player"]==boundary["player"] && p["kind"]==boundary["kind"]))
            .map(|mut boundary| {
                let question=quoted.asked();
                boundary["asked"]=render_asked(&quoted,question);
                boundary["announced"]=quoted.audit_game_objects()["objects"][boundary["source"].to_string()]["announced"].clone();
                boundary
            })
    } else {None};
    let suspended = suspended.or_else(|| quoted.payment_mana_boundary().map(|mut boundary| {
        let question=quoted.asked();
        boundary["asked"] = render_asked(&quoted, question);
        boundary["announced"] = quoted.audit_game_objects()["objects"][boundary["source"].to_string()]["announced"].clone();
        boundary
    }));
    let symbol_indices=(|| -> Option<Vec<usize>> {
        if payments.last()?["path"] != json!([]) {return None;}
        let final_payment=payments.last()?;
        let locked=final_payment["requirement"]["symbols"].as_array()?.iter().map(|s|s.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()?;
        let life=match request.get("life").and_then(Value::as_array) {Some(v)=>v.iter().map(Value::as_bool).collect::<Option<Vec<_>>>()?,None=>Vec::new()};
        let x=request.get("x").and_then(Value::as_u64).map(|n|n as u32);
        match request["kind"].as_str()? {
            "cast"=>{
                if request.get("cost").is_some_and(|c|c!="mana" && !c.is_null())
                    || request.get("route").is_some_and(|r|r!="normal" && !r.is_null()) {return None;}
                let cast_as=match request.get("cast_as") {
                    None=>CastAs::Whole,Some(v) if v=="whole"=>CastAs::Whole,
                    Some(v) if v=="adventure"=>CastAs::Adventure,Some(v) if v=="omen"=>CastAs::Omen,
                    Some(v) if v.get("face").is_some()=>CastAs::Face(v["face"].as_u64()? as usize),
                    Some(v) if v.as_u64().is_some()=>CastAs::Half(v.as_u64()? as usize),_=>return None,
                };
                g.payment_symbol_indices(read_id(&request["card"]).ok()?,None,cast_as,&life,x,&locked)
            }
            "activate"=>g.payment_symbol_indices(read_id(&request["source"]).ok()?,Some(request["ability"].as_u64()? as usize),CastAs::Whole,&life,x,&locked),
            _=>None,
        }
    })();
    let symbol_indices=symbol_indices.or_else(|| {
        let symbols=payments.last()?["requirement"]["symbols"].as_array()?.iter()
            .map(|s|s.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()?;
        g.pending_payment_symbol_indices(&symbols)
    });
    let payment_pool=quoted.quoted_payment_pools();
    let mut proof=json!({"move":request,"complete":complete,"error":outcome.as_ref().err().map(ToString::to_string),"payments":payments,"events":events,
        "symbol_indices":symbol_indices,"productions":quoted.payment_production_audit(),"mana_answers":quoted.payment_mana_answers(),
        "prior_pools":g.players.iter().map(|p|Game::audit_payment_pool(&p.pool)).collect::<Vec<_>>(),
        "pools":quoted.players.iter().map(|p|Game::audit_payment_pool(&p.pool)).collect::<Vec<_>>()});
    // the quoted move's refusal says its kind, as the same move's would (`error_class`, and
    // of an illegal one the rules it cites, `error_rules`)
    if let Err(refused)=&outcome {
        proof["error_class"]=json!(refused.class().name());
        if refused.class()==Class::Illegal {proof["error_rules"]=json!(game::simulation::cited_rules(&refused.to_string()));}
    }
    if let Some(boundary)=suspended {proof["suspended"]=boundary;}
    if let Some(boundary)=g.payment_mana_boundary() { proof["continuation"] = boundary; }
    PaymentProbe { proof, pools:payment_pool, outcome }
}

fn payment_quote(g:&Game, player:usize, request:&Value) -> Result<Value,Error> {
    payment_probe(g,player,request).map(|probe|probe.proof)
}

/// Resolve only caller-supplied restricted indices at their actual cost boundary.
/// Probes omit that payment and stop unpaid; they never choose mana to advance it.
fn read_payment_move(g:&Game,player:usize,request:&Value,pool:&Pool,offered:&[game::operation::ReplacementId]) -> Result<Move,Error> {
    if let Some(pending)=request.get("pending_payment") {
        let fields=pending.as_object().ok_or("pending_payment is an exact scope-bound selector")?;
        if fields.len()!=3 || !fields.contains_key("source") || !fields.contains_key("path") || !fields.contains_key("mana") || pending["path"]!=json!([]) {
            return Err("pending_payment names its exact source, root path [], and mana selector".into());
        }
        let mut plain=request.clone();plain.as_object_mut().unwrap().remove("pending_payment");
        let Move::Choose(game::state::Answer::Mana(adding))=read_payment_move(g,player,&plain,pool,offered)? else {
            return Err("pending_payment accompanies only the real triggered-mana answer".into());
        };
        let source=read_id(&pending["source"])?;
        let mut selector=pending["mana"].clone();
        let mana=if let Some(indices)=selector.get("restricted_after").cloned() {
            if selector.get("restricted").is_some() {return Err("pending payment chooses only one restricted boundary".into());}
            let plain_move=Move::Choose(game::state::Answer::Mana(adding.clone()));
            let probe=payment_probe_move(g,player,&plain,plain_move);
            let index=match probe.proof["payments"].as_array().and_then(|rs|rs.iter().position(|r|r["path"]==json!([]) && r["source"]==source.0)) {
                Some(index)=>index,
                None=>{
                    probe.outcome.map_err(ResponseError::from)?;
                    return Err("pending selector did not reach the exact post-production root boundary".into());
                }
            };
            let boundaries=probe.pools;
            selector.as_object_mut().ok_or("pending mana is an object")?.remove("restricted_after");
            selector["restricted"]=indices;
            read_pool_selection(&selector,&boundaries[index])?
        } else {read_pool_selection(&selector,pool)?};
        return Ok(Move::ContinueManaPayment {source,mana,adding});
    }
    let payment_path = if request["kind"] == "choose" && request.pointer("/answer/pay").is_some() { "/answer/pay" } else { "" };
    fn nodes(value:&Value, pointer:String, path:Vec<usize>, out:&mut Vec<(String,Vec<usize>)>) {
        if let Some(plan)=value.get("plan").and_then(Value::as_array) {
            for (i,child) in plan.iter().enumerate() {let mut child_path=path.clone();child_path.push(i);nodes(child,format!("{pointer}/plan/{i}"),child_path,out);}
        }
        out.push((pointer,path));
    }
    let mut nodes_list=Vec::new();
    nodes(request.pointer(payment_path).ok_or("missing payment")?,payment_path.into(),Vec::new(),&mut nodes_list);
    let mut selected=request.clone();
    let mut pools=std::collections::BTreeMap::new();
    let mut outer=pool.clone();
    for (pointer,path) in &nodes_list {
        let Some(mana)=request.pointer(&format!("{pointer}/mana")) else {continue};
        if mana.get("restricted").is_none() && mana.get("restricted_after").is_none() {continue;}
        if mana.get("restricted").is_some() && mana.get("restricted_after").is_some() {return Err("choose either restricted or restricted_after indices".into());}
        let mut baseline=selected.clone();
        // Strip unresolved selectors only; their identities are unavailable until
        // the caller's exact prefix reaches their own scoped locked boundary.
        for (pending,pending_path) in &nodes_list {
            if pending_path==path || (!pending_path.is_empty() && !pools.contains_key(pending_path)) || pending_path.is_empty() {
                if let Some(object)=baseline.pointer_mut(pending).and_then(Value::as_object_mut) {
                    if object.get("mana").is_some_and(|m|m.get("restricted").is_some() || m.get("restricted_after").is_some()) {object.remove("mana");}
                }
            }
        }
        let mv=read_move_at_pools(&baseline,&outer,offered,&pools)?;
        let probe=payment_probe_move(g,player,&baseline,mv);
        let index=match probe.proof["payments"].as_array().and_then(|rs|rs.iter().position(|r|r["path"]==json!(path))) {
            Some(index)=>index,
            None=>{
                probe.outcome.map_err(ResponseError::from)?;
                return Err("restricted selection did not reach its scoped locked boundary".into());
            }
        };
        let (quote,boundaries)=(probe.proof,probe.pools);
        let boundary=boundaries.get(index).ok_or("missing scoped boundary pool")?;
        let chosen_pool=if mana.get("restricted_after").is_some() {boundary.clone()} else {
            // Restricted indices before a child's mana window name its initial pool.
            // Resolve that same identity through the quoted live pool.
            let prior=&quote["payments"][index]["prior_pool"];
            let indices=mana["restricted"].as_array().ok_or("restricted selector must be an array")?;
            let current=&quote["payments"][index]["pool"]["restricted"];
            let mut used=std::collections::BTreeSet::new();
            let mapped=indices.iter().map(|i| {
                let unit=prior["restricted"].get(i.as_u64().ok_or("restricted index must be an integer")? as usize).ok_or("restricted index outside initial payment pool")?;
                current.as_array().and_then(|units|units.iter().enumerate().find(|(i,u)|!used.contains(i) && *u==unit).map(|(i,_)|i)).map(|n|{used.insert(n);json!(n)}).ok_or("selected restricted mana was consumed by a child")
            }).collect::<Result<Vec<_>,&str>>()?;
            selected.pointer_mut(&format!("{pointer}/mana")).unwrap()["restricted"]=json!(mapped);
            boundary.clone()
        };
        if let Some(indices)=mana.get("restricted_after") {let object=selected.pointer_mut(&format!("{pointer}/mana")).and_then(Value::as_object_mut).ok_or("mana must be an object")?;object.remove("restricted_after");object.insert("restricted".into(),indices.clone());}
        if path.is_empty() {outer=chosen_pool;} else {pools.insert(path.clone(),chosen_pool);}
    }
    read_move_at_pools(&selected,&outer,offered,&pools)
}

/// State-dependent decoding can execute isolated payment probes. Hosts call it
/// inside Simulation::observe so every such rules query shares its fault boundary.
fn read_response_move(g:&Game, player:usize, request:&Value) -> Result<Move,Error> {
    let pool=&g.players.get(g.decision_actor()).ok_or_else(|| Error::unreferable("no such player"))?.pool;
    let offered=match &g.decision {
        Some(game::state::Decision::Choose {choice:game::state::Choice::Replacement {among,..},..})=>among.as_slice(),
        _=>&[],
    };
    optional_named(request,g)?;
    match &g.decision {
        _ if request["kind"]=="concede"=>read_move_offered(request,pool,offered),
        Some(game::state::Decision::Choose {choice:game::state::Choice::DamageOrder {among,..},..})=>read_damage_order(request,among),
        Some(game::state::Decision::Choose {choice:game::state::Choice::DamageReplacement {among,..},..})=>read_damage_move(request,among),
        _=>read_payment_move(g,player,request,pool,offered),
    }
}

/// Exact observations from an isolated modal announcement, never the live host.
fn modal_capture_frame(faces:&Catalog,game:&mut Option<Simulation>,session:&mut u64,logged:&mut Delivery,
    state:Value,actor:Option<usize>)->Result<Value,Error> {
    let audit=dispatch(faces,game,session,logged,&json!({"cmd":"audit_game","session_id":*session}))?;
    let count=game.as_ref().ok_or_else(|| Error::unreferable("no game"))?.game()?.players.len();
    let mut libraries=Vec::new();
    for player in 0..count {
        libraries.push(dispatch(faces,game,session,logged,&json!({"cmd":"library_order","session_id":*session,"player":player}))?);
    }
    let mut frame=json!({"state":state,"audit":audit,"libraries":libraries});
    if let Some(actor)=actor {
        let prompt=dispatch(faces,game,session,logged,&json!({"cmd":"prompt","seat":actor}))?;
        frame["prompt"]=prompt;
        let labels=dispatch(faces,game,session,logged,&json!({"cmd":"labels","seat":actor}))?;
        frame["labels"]=labels["labels"].clone();
    }
    Ok(frame)
}

/// Only request correlations differ when one announcement uses two responses.
fn modal_game_facts(mut frame:Value)->Value {
    if let Some(state)=frame["state"].as_object_mut() { state.remove("request_id"); }
    if let Some(asked)=frame["state"]["asked"].as_object_mut() { asked.remove("request_id"); }
    frame
}

/// The pipe routes one simulation at a time. Session IDs prevent a delayed response from
/// an earlier `new`/`start` being accepted at the same game-local request number.
fn dispatch(
    faces: &Catalog,
    game: &mut Option<Simulation>,
    session_id: &mut u64,
    logged: &mut Delivery,
    input: &Value,
) -> Result<Value, Error> {
    let fields: &[&str] = match input["cmd"].as_str().unwrap_or("") {
        "new" => &["players", "life", "viewer", "ask_forced", "position"],
        "start" => &["decks", "companions", "commanders", "format", "sideboards", "life", "first", "seed", "orders", "viewer", "ask_forced"],
        "act" => &["session_id", "request_id", "player", "move", "viewer", "payment_expect", "capture_random", "random_outcomes", "random_outcomes_keep_unused", "shuffles", "shuffles_keep_unused"],
        "quote_payment" | "quote_modal" => &["session_id", "request_id", "player", "move", "viewer"],
        "state" => &["viewer"],
        "version" => &[],
        "audit_initial" | "audit_pending" | "audit_game" | "audit_objects" => &["session_id", "viewer"],
        "library_order" => &["session_id", "player", "viewer"],
        "set_library_order" => &["session_id", "request_id", "player", "order", "viewer"],
        "labels" | "prompt" => &["session_id", "seat"],
        "answer" => &["session_id", "request_id", "seat", "answer"],
        _ => &[],
    };
    request_fields(input, fields)?;
    let mut captured_random = None;
    let mut shuffles_unused = None;
    let mut random_outcomes_unused = None;
    let mut random_outcomes_unused_indices = None;
    match input.get("cmd").and_then(Value::as_str).unwrap_or("") {
        "new" | "start" => {
            return reset(game, session_id, logged, input, || {
                if input["cmd"] == "new" { new_game(faces, input) } else { start_game(faces, input) }
            });
        }
        "quote_modal" => {
            request_fields(input, &["session_id", "request_id", "player", "move", "viewer"])?;
            if input.get("viewer").is_some() { return Err("quote_modal is privileged".into()); }
            if input["session_id"].as_u64()!=Some(*session_id) { return Err(Error::stale("stale session_id")); }
            let mut split=game.as_ref().ok_or_else(|| Error::unreferable("no game"))?.clone();
            let pending=split.request()?.ok_or_else(|| Error::stale("no pending request"))?.clone();
            if input["request_id"].as_u64()!=Some(pending.id.0) { return Err(Error::stale("stale request_id")); }
            let actor=input["player"].as_u64().ok_or("the request has no player")? as usize;
            if actor!=pending.player { return Err(Error::unreferable("another player's request")); }
            if !matches!(split.game()?.decision,Some(game::state::Decision::Modes { .. })) {
                return Err("quote_modal needs a triggered-ability modes boundary".into());
            }
            let command=input.get("move").ok_or("the request has no move")?;
            if command["kind"]!="targets" || command.get("modes").and_then(Value::as_array).is_none()
                || command.get("targets").and_then(Value::as_array).is_none_or(Vec::is_empty)
                || command.as_object().is_none_or(|fields|fields.keys().any(|k|!matches!(k.as_str(),"kind"|"modes"|"targets"|"division"))) {
                return Err("quote_modal needs the exact combined modes and targets command".into());
            }
            let modes=json!({"kind":"modes","modes":command["modes"]});
            let mut targets=command.clone();
            targets.as_object_mut().unwrap().remove("modes");
            let mut split=Some(split);
            let mut joined=game.clone();
            let (mut split_log,mut joined_log)=(logged.clone(),logged.clone());
            let (mut split_session,mut joined_session)=(*session_id,*session_id);
            let act=|id:Value,mv:&Value|json!({"cmd":"act","session_id":*session_id,"request_id":id,
                "player":actor,"move":mv,"capture_random":true});
            let Move::ChooseModes(indices)=read_move_offered(&modes,&Pool::default(),&[])? else { unreachable!() };
            split_log.snapshot(split.as_ref().unwrap().game()?);
            split.as_mut().unwrap().probe_modal_modes(pending.id,actor,indices)?;
            let mut middle=dispatch(faces,&mut split,&mut split_session,&mut split_log,&json!({"cmd":"state"}))?;
            middle["random_outcomes"]=json!([]);
            if middle["asked"]["kind"]!="targets" || middle["random_outcomes"]!=json!([]) {
                return Err(Error::unsupported("modal split has no target-only intermediate boundary"));
            }
            let middle=modal_capture_frame(faces,&mut split,&mut split_session,&mut split_log,middle,Some(actor))?;
            let split_final=dispatch(faces,&mut split,&mut split_session,&mut split_log,&act(middle["state"]["request_id"].clone(),&targets))?;
            let split_final=modal_capture_frame(faces,&mut split,&mut split_session,&mut split_log,split_final,None)?;
            let joined_final=dispatch(faces,&mut joined,&mut joined_session,&mut joined_log,&act(json!(pending.id.0),command))?;
            let joined_final=modal_capture_frame(faces,&mut joined,&mut joined_session,&mut joined_log,joined_final,None)?;
            if modal_game_facts(split_final)!=modal_game_facts(joined_final.clone()) {
                return Err(Error::unsupported("split modal announcement changes combined-command game facts"));
            }
            return Ok(json!({"move":command,"session_id":*session_id,"request_id":pending.id.0,"player":actor,
                "intermediate":middle,"final":joined_final,"equivalent":true,
                "transport_ignored":["state.request_id","state.asked.request_id"]}));
        }
        "quote_payment" => {
            request_fields(input, &["session_id", "request_id", "player", "move", "viewer"])?;
            if input.get("viewer").is_some() {return Err("quote_payment is privileged".into());}
            if input["session_id"].as_u64()!=Some(*session_id) {return Err(Error::stale("stale session_id"));}
            let simulation=game.as_mut().ok_or_else(|| Error::unreferable("no game"))?;
            let pending=simulation.request()?.ok_or_else(|| Error::stale("no pending request"))?;
            if input["request_id"].as_u64()!=Some(pending.id.0) {return Err(Error::stale("stale request_id"));}
            let player=input["player"].as_u64().ok_or("the request has no player")? as usize;
            if player!=pending.player {return Err(Error::unreferable("another player's request"));}
            let request=input.get("move").ok_or("the request has no move")?;
            return simulation.observe(|g|payment_quote(g,player,request))?;
        }
        "act" => {
            request_fields(input, &["session_id", "request_id", "player", "move", "viewer", "payment_expect", "capture_random",
                "random_outcomes", "random_outcomes_keep_unused", "shuffles", "shuffles_keep_unused"])?;
            let (capture, outcomes_keep_unused, shuffles_keep_unused) =
                (flag(input, "capture_random")?, flag(input, "random_outcomes_keep_unused")?, flag(input, "shuffles_keep_unused")?);
            let simulation = game.as_mut().ok_or_else(|| Error::unreferable("no game"))?;
            let session = input["session_id"].as_u64().ok_or("the request has no session_id")?;
            if session != *session_id { return Err(Error::stale("stale session_id")); }
            let id = RequestId(input["request_id"].as_u64().ok_or("the request has no request_id")?);
            let player = input["player"].as_u64().ok_or("the request has no player")? as usize;
            let request=input.get("move").ok_or("the request has no move")?;
            if request["kind"] != "concede" {
                simulation.validate_request(id, player)?;
            }
            let mv=simulation.observe(|g| -> Result<Move,Error> {
                if let Some(expected)=input.get("payment_expect") {
                    let actual=payment_quote(g,player,request)?;
                    let refused="payment quote changed or is incomplete; action was not applied";
                    if &actual!=expected { return Err(Error::stale(refused)); }
                    // Apply exactly a complete quote or its retained suspension.
                    if actual["complete"]!=true && actual.get("suspended").is_none() { return Err(refused.into()); }
                }
                let mv=read_response_move(g,player,request)?;
                let mv=if input.pointer("/payment_expect/suspended/stage").and_then(Value::as_str)==Some("mana_choice") {
                    Move::AuditPayment(Box::new(mv))
                } else {mv};
                read_viewer(g, input)?;
                Ok(mv)
            })??;
            if input.get("shuffles_keep_unused").is_some() && input.get("shuffles").is_none() {
                return Err("replay input: shuffles_keep_unused needs shuffles".into());
            }
            if input.get("random_outcomes_keep_unused").is_some() && input.get("random_outcomes").is_none() {
                return Err("replay input: random_outcomes_keep_unused needs random_outcomes".into());
            }
            if input.get("random_outcomes").is_some() && input.get("shuffles").is_some() {
                return Err("replay input: random_outcomes and shuffles are exclusive".into());
            }
            if capture && input.get("viewer").is_some() { return Err("capture_random is privileged, not a viewer projection".into()); }
            if capture && (input.get("shuffles").is_some() || input.get("random_outcomes").is_some()) { return Err("capture_random and replay outcomes are exclusive".into()); }
            if input.get("random_outcomes").is_some() && input.get("viewer").is_some() { return Err("random_outcomes is privileged".into()); }
            let outcomes = input.get("random_outcomes").map(read_random_outcomes).transpose()?;
            let shuffles = input.get("shuffles").map(read_shuffles).transpose().map_err(|why| game::start::ReplayInput(why).to_string())?;
            // what happened so far is rendered before the game goes on and may forget it
            logged.snapshot(simulation.game()?);
            #[cfg(feature = "profile")]
            let _core = game::profile::span(game::profile::Scope::Respond);
            if capture {
                let outcomes=simulation.respond_capturing_random(id,player,mv)?;
                captured_random=Some(outcomes.into_iter().map(|outcome| match outcome {
                    game::start::CapturedRandom::Order {kind,player,position,order} => random_outcome(kind,Some(player),position,"physical_order",json!(order)),
                    game::start::CapturedRandom::Coin {player,face} => json!({"kind":"coin","player":player,"result":face.name()}),
                }).collect::<Vec<_>>());
            } else if let Some(outcomes) = outcomes {
                let unused = simulation.respond_with_random(id, player, mv, outcomes, outcomes_keep_unused)?;
                if outcomes_keep_unused {
                    random_outcomes_unused_indices = Some(unused.iter().map(|unused|unused.input_index).collect::<Vec<_>>());
                    random_outcomes_unused = Some(unused.into_iter().map(|unused| {
                        match unused.outcome {
                            game::start::RandomOutcome::Shuffle {player,order} => random_outcome(game::start::RandomKind::Shuffle,Some(player),None,"order",json!(order.iter().map(|id|id.0).collect::<Vec<_>>())),
                            game::start::RandomOutcome::RandomOrder {player,order} => random_outcome(game::start::RandomKind::RandomOrder,player,None,"order",json!(order.iter().map(|id|id.0).collect::<Vec<_>>())),
                            game::start::RandomOutcome::Coin {player,face} => json!({"kind":"coin","player":player,"result":face.name()}),
                        }
                    }).collect::<Vec<_>>());
                }
            } else if let Some(outcomes) = shuffles {
                if shuffles_keep_unused {
                    let unused=simulation.respond_with_shuffles_keeping_unused(id, player, mv, outcomes)?;
                    shuffles_unused=Some(unused.into_iter().map(|outcome|json!({"player":outcome.player,
                        "order":outcome.order.iter().map(|id|id.0).collect::<Vec<_>>()})).collect::<Vec<_>>());
                } else {
                    simulation.respond_with_shuffles(id, player, mv, outcomes)?;
                }
            } else {
                simulation.respond(id, player, mv)?;
            }
        }
        "audit_initial" => {
            if input.get("viewer").is_some() { return Err("audit_initial is privileged".into()); }
            let simulation=game.as_ref().ok_or_else(|| Error::unreferable("no game"))?;
            if input["session_id"].as_u64()!=Some(*session_id) {return Err(Error::stale("stale session_id"));}
            return simulation.game()?.audit_initial_history().map_err(Error::unsupported);
        }
        "audit_pending" => {
            if input.get("viewer").is_some() { return Err("audit_pending is privileged".into()); }
            let simulation=game.as_ref().ok_or_else(|| Error::unreferable("no game"))?;
            if input["session_id"].as_u64()!=Some(*session_id) { return Err(Error::stale("stale session_id")); }
            return Ok(simulation.game()?.audit_pending_trigger_order());
        }
        "audit_game" => {
            if input.get("viewer").is_some() { return Err("audit_game is privileged".into()); }
            let simulation=game.as_ref().ok_or_else(|| Error::unreferable("no game"))?;
            if input["session_id"].as_u64()!=Some(*session_id) { return Err(Error::stale("stale session_id")); }
            return Ok(simulation.game()?.audit_game_objects());
        }
        // Opt-in audit projection: no work or additional hidden fields in player prompts.
        "audit_objects" => {
            if input.get("viewer").is_some() { return Err("audit_objects is privileged".into()); }
            let simulation = game.as_ref().ok_or_else(|| Error::unreferable("no game"))?;
            let session = input["session_id"].as_u64().ok_or("the request has no session_id")?;
            if session != *session_id { return Err(Error::stale("stale session_id")); }
            let g = simulation.game()?;
            let mut objects = BTreeMap::new();
            for id in g.battlefield.iter().chain(g.stack.iter()) {
                let mut value = json!({});
                if g.battlefield.contains(id) {
                    let seen = g.seen(*id, None);
                    value["keywords"] = json!(seen.keywords.iter().map(|held|held.keyword.name().to_lowercase()).collect::<Vec<_>>());
                }
                if let Some((source, _)) = g.obj(*id).ability {
                    value["source"] = json!(source.map(oid));
                    // the text it was put on the stack with (405.4, 113.7a), not what its
                    // source has now
                    if let Some(text) = g.stack_ability_text(*id) { value["text"] = json!(text); }
                }
                objects.insert(id.0.to_string(), value);
            }
            return Ok(json!({"objects":objects}));
        }
        "library_order" | "set_library_order" => {
            if input["cmd"] == "library_order" { request_fields(input, &["session_id", "player", "viewer"])?; }
            else { request_fields(input, &["session_id", "request_id", "player", "order", "viewer"])?; }
            let simulation = game.as_mut().ok_or_else(|| Error::unreferable("no game"))?;
            let session = input["session_id"].as_u64().ok_or("the request has no session_id")?;
            if session != *session_id { return Err(Error::stale("stale session_id")); }
            let player = input["player"].as_u64().ok_or("the request has no player")? as usize;
            if input["cmd"] == "library_order" {
                if input.get("viewer").is_some() { return Err("library_order is a privileged observation, not a viewer projection".into()); }
                let g = simulation.game()?;
                let library = &g.players.get(player).ok_or_else(|| Error::unreferable("no such player"))?.library;
                let identities: BTreeMap<_, _> = library.iter().filter_map(|id|
                    g.physical_card_identity(*id).map(|card| (id.0.to_string(), card))).collect();
                return Ok(json!({"player":player,"order":library.iter().rev().map(|id|id.0).collect::<Vec<_>>(),
                                 "identities":identities}));
            }
            let id = RequestId(input["request_id"].as_u64().ok_or("the request has no request_id")?);
            let order = read_library_order(&input["order"])?;
            // the order is put only once everything else the request says is known good
            let g = simulation.game()?;
            if player >= g.players.len() { return Err(Error::unreferable("no such player")); }
            read_viewer(g, input)?;
            simulation.set_library_order(id, player, &order)?;
        }
        "state" => {}
        "version" => return Ok(json!({ "version": version() })),
        "labels" => {
            let simulation = game.as_ref().ok_or_else(|| Error::unreferable("no game"))?;
            let seat = read_seat(simulation.game()?, input)?;
            let given = simulation.labels(seat).map(|l| l.given().map(|(n, id)| (format!("o{n}"), json!(id.0))).collect()).unwrap_or_default();
            return Ok(json!({ "labels": Value::Object(given) }));
        }
        "prompt" | "answer" => return player(game, *session_id, logged, input),
        other => return Err(format!("unknown command \"{other}\"").into()),
    }
    let mut state = reply_state(game.as_mut().ok_or_else(|| Error::unreferable("no game"))?, *session_id, logged, input)?;
    if let Some(outcomes)=captured_random { state["random_outcomes"]=json!(outcomes); }
    if let Some(unused)=shuffles_unused { state["shuffles_unused"]=json!(unused); }
    if let Some(unused)=random_outcomes_unused { state["random_outcomes_unused"]=json!(unused); }
    if let Some(indices)=random_outcomes_unused_indices { state["random_outcomes_unused_indices"]=json!(indices); }
    Ok(state)
}

fn reply_state(simulation: &mut Simulation, session_id: u64, logged: &mut Delivery, input: &Value) -> Result<Value, Error> {
    let pending = simulation.request()?.cloned();
    let seat = simulation.observe(|g| read_viewer(g, input))??.seat(pending.as_ref().map(|r| r.player));
    let shown = match seat {
        Some(seat) => simulation.take_observations(seat)?,
        None => Vec::new(),
    };
    #[cfg(feature = "profile")]
    let _projection = game::profile::span(game::profile::Scope::Projection);
    // the game as this viewer may see it: the one already taken, if it has not gone on
    let seen = simulation.snapshot(seat)?;
    simulation.observe(|g| {
        logged.snapshot(g);
        let viewer = seat;
        let request_id = pending.as_ref().map(|r| r.id.0);
        let asking = render_asked(g, pending.map(|r| (r.acting, r.question)));
        let mut state = render_snapshot(&seen, asking, logged);
        state["session_id"] = json!(session_id);
        state["request_id"] = json!(request_id);
        if viewer.is_some() {
            state["observations"] = shown.iter().map(|d| disclosure(d)).collect();
        }
        if !state["asked"].is_null() {
            state["asked"]["request_id"] = json!(request_id);
        }
        state
    }).map_err(Error::from)
}

/// The seat a player command is for: `"seat":"p0"` or `"seat":0`.
/// A yield as a seat asks for it: `"until":"end_of_turn"`, `"until":{"part":"End","whose":"p1"}`
/// (a part as `part` says it; `whose` optional), or `"until":null` to stop yielding; with
/// `"stop_on_stack"` (default true).




fn read_seat(g: &Game, input: &Value) -> Result<usize, Error> {
    let seat = match input.get("seat") {
        Some(Value::String(label)) => match Label::parse(label) { Some(Label::Player(p)) => p, _ => return Err(format!("a seat is a player label, not {label}").into()) },
        Some(n) => n.as_u64().ok_or("a seat is a player label or number")? as usize,
        None => return Err("a player command says its \"seat\"".into()),
    };
    if seat >= g.players.len() { return Err(Error::unreferable(format!("the game has no player {seat}"))); }
    Ok(seat)
}

/// The player-facing commands (`game::prompt`, API `player/1`): what a player may send.
/// `prompt` says the game as that seat may see it, with labels, readable text and the
/// question if it is theirs; `answer` answers it with labels. Neither can ask for another
/// seat's view or for the whole game, and neither lists moves.
fn player(game: &mut Option<Simulation>, session_id: u64, logged: &mut Delivery, input: &Value) -> Result<Value, Error> {
    if input.get("shuffles").is_some() || input.get("random_outcomes").is_some() { return Err("random outcomes belong to the privileged act command".into()); }
    let simulation = game.as_mut().ok_or_else(|| Error::unreferable("no game"))?;
    if let Some(session) = input.get("session_id") {
        if session.as_u64() != Some(session_id) { return Err(Error::stale("stale session_id")); }
    } else if input["cmd"] != "prompt" {
        return Err("the request has no session_id".into());
    }
    let seat = read_seat(simulation.game()?, input)?;
    if input["cmd"] == "answer" {
        let id = RequestId(input["request_id"].as_u64().ok_or("the request has no request_id")?);
        let mut request = input.get("answer").ok_or("the request has no answer")?.clone();
        let names = simulation.labels(seat).cloned().unwrap_or_default();
        let mv = match unname_objects(&mut request, &names) {
            Err(why) => Err(why),
            Ok(()) => {
                let request = &request;
                simulation.observe(|g| labelled(|| read_response_move(g,seat,request)))?
            }
        };
        logged.snapshot(simulation.game()?);
        match mv {
            Err(why) => return Err(why),
            Ok(mv) => match simulation.answer(seat, id, mv) {
                Ok(()) => {}
                // an answer names only what this seat was given a name for, and the players there are
                Err(ResponseError::Unreferable(named)) => return Err(Error::unreferable(format!("{} names nothing you can refer to now", match named {
                    Target::Object(o) => names.number(o).map_or("the answer".into(), |n| Label::Object(n).to_string()),
                    Target::Player(p) => Label::Player(p).to_string(),
                }))),
                Err(e) => return Err(e.into()),
            },
        }
    }
    // a seat's events are kept for its prompts from the first one on
    logged.snapshot(simulation.game()?);
    logged.prompted.entry(seat).or_default();
    let shown = simulation.take_observations(seat)?;
    let prompt = simulation.prompt(seat)?;
    let mut said = simulation.observe(|g| render_prompt(g, &prompt, session_id, logged, &shown))?;
    name_objects(&mut said, simulation.labels_mut(seat));
    Ok(said)
}

/// A spell or ability as it occurred (`game::prompt::Occurrence`).
fn occurrence_json(o: &game::prompt::Occurrence) -> Value {
    let mut v = json!({ "kind": o.kind.name(), "controller": pid(o.controller) });
    if let Some(object) = o.object { v["object"] = oid(object); }
    if let Some(source) = o.source { v["source"] = oid(source); }
    if !o.source_name.is_empty() { v["source_name"] = json!(o.source_name); }
    if o.source_left { v["source_left"] = json!(true); }
    if let Some(text) = &o.text { v["text"] = json!(text); }
    if let Some(text) = &o.created_by { v["created_by"] = json!(text); }
    if o.granted { v["granted"] = json!(true); }
    if !o.modes.is_empty() { v["modes"] = o.modes.iter().map(|(index, text)| json!({ "index": index, "text": text })).collect(); }
    v
}

/// A player's prompt as the pipe says it, with labels: the game as the seat may see it
/// ([`render_view`]), each object's text and labelled activated abilities, and the question
/// when it is this seat's, with whom it is asked of and why. What was delivered once since
/// the seat was last prompted comes with it: events, what it was shown, the answers the
/// engine chose for it.
fn render_prompt(g: &Game, prompt: &game::prompt::Prompt, session_id: u64, logged: &mut Delivery , shown: &[std::sync::Arc<Disclosure>]) -> Value {
    labelled(|| {
        let seat = prompt.seat;
        let mut v = render_view(&prompt.view);
        if let Some(objects) = v["objects"].as_object_mut() {
            for (id, entity) in &prompt.entities {
                let Some(o) = objects.get_mut(&key(*id)) else { continue };
                let o = o.as_object_mut().expect("an object is said as a map");
                o.remove("activated");
                o.insert("zone".into(), json!(zone_name(g.obj(*id).zone)));
                if let Some(cost) = &entity.mana_cost { o.insert("mana_cost".into(), json!(cost)); }
                o.insert("text".into(), json!(entity.text));
                if !entity.abilities.is_empty() {
                    o.insert("abilities".into(), entity.abilities.iter().map(|a| {
                        let mut ability = json!({ "label": aid(*id, a.index), "text": a.text });
                        if let Some(by) = a.granted { ability["granted_by"] = by.map_or(json!("an effect"), oid); }
                        ability
                    }).collect());
                }
                if let Some(card) = &entity.card { o.insert("card_text".into(), json!(card)); }
                // the inset spell it has without using it (715.2a, 720.2a, 722.2a)
                if let Some(inset) = &entity.inset {
                    o.insert(inset.kind.name().into(), json!({
                        "name": inset.name,
                        "mana_cost": inset.mana_cost,
                        "supertypes": inset.supertypes.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>(),
                        "types": inset.types.iter().map(|t| format!("{t:?}")).collect::<Vec<_>>(),
                        "subtypes": inset.subtypes.iter().map(|s| s.name()).collect::<Vec<_>>(),
                        "colors": inset.colors.iter().map(|c| c.word()).collect::<Vec<_>>(),
                        "text": inset.text,
                    }));
                }
                if let Some(occurrence) = &entity.occurrence { o.insert("occurrence".into(), occurrence_json(occurrence)); }
            }
        }
        if let Some(players) = v["players"].as_array_mut() {
            for (index, p) in players.iter_mut().enumerate() { p["you"] = json!(index == seat); }
        }
        let map = v.as_object_mut().expect("a view is a map");
        map.remove("viewer");
        map.insert("api".into(), json!(game::prompt::API));
        map.insert("session_id".into(), json!(session_id));
        map.insert("request_id".into(), json!(prompt.request.map(|r| r.0)));
        map.insert("seat".into(), pid(seat));
        map.insert("waiting_for".into(), prompt.waiting_for.map_or(Value::Null, pid));
        map.insert("decision".into(), prompt.decision.as_ref().map_or(Value::Null, |d| {
            let mut q = render_asked(g, Some((d.acting, d.question.clone())));
            let question = q.as_object_mut().expect("a question is a map");
            question.remove("player");
            question.remove("entering");
            question.insert("request_id".into(), json!(d.id.0));
            question.insert("chooser".into(), pid(d.chooser));
            if d.acting != d.chooser { question.insert("acting".into(), pid(d.acting)); } else { question.remove("acting"); }
            question.insert("reason".into(), json!(d.reason.name()));
            if let Some(o) = &d.occurrence { question.insert("for".into(), occurrence_json(o)); }
            if !d.ordered.is_empty() { question.insert("ordered".into(), d.ordered.iter().map(occurrence_json).collect()); }
            if let Some(card) = d.entering { question.insert("entering".into(), oid(card)); }
            q
        }));
        map.insert("events".into(), logged.take_prompted(seat).into());
        map.insert("observations".into(), shown.iter().map(|d| disclosure(d)).collect());
        v
    })
}

#[path = "game-serve/replay.rs"]
mod replay;
#[path = "game-serve/parallel.rs"]
mod parallel;
#[path = "game-serve/host.rs"]
mod host;
#[path = "game-serve/native.rs"]
mod native;

fn main() {
    let catalog_began = std::time::Instant::now();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut faces = corpus::load(&corpus::default_path()).unwrap_or_else(|e| panic!("cannot read the corpus {}: {e}", corpus::default_path().display()));
    // Explicit diagnostic fixtures supplement the pinned corpus; they cannot shadow
    // native definitions. This also lets matched replay use the exact synthetic input.
    if args.first().is_some_and(|a| a == "--fixture-corpus") {
        let path = args.get(1).expect("--fixture-corpus needs a JSONL path");
        let fixtures = corpus::load(std::path::Path::new(path)).expect("cannot read fixture corpus");
        assert!(fixtures.iter().all(|f| f.oracle_id.starts_with("fixture:") && !faces.iter().any(|native| native.name == f.name)),
            "fixture identities must be labeled and cannot shadow native cards");
        faces.extend(fixtures);
        args.drain(..2);
    }
    let by_name = Catalog::new(CardDef::catalog(&faces));
    let catalog_load_seconds = catalog_began.elapsed().as_secs_f64();
    if args.first().is_some_and(|a| a == "--bench-native") {
        let result=(|| -> Result<Value,String> {
            let config:Value=serde_json::from_str(&std::fs::read_to_string(args.get(1).ok_or("need config path")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            let mut report=native::run(&by_name,&config)?;
            report["cold_catalog_load_seconds"]=json!(catalog_load_seconds);
            Ok(report)
        })();
        match result {Ok(report)=>println!("{report}"),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}
        return;
    }
    if args.first().is_some_and(|a| a == "--host") {
        let workers = args.get(1).and_then(|s| s.parse::<usize>().ok()).filter(|n| (1..=256).contains(n)).unwrap_or_else(|| { eprintln!("usage: game-serve --host WORKERS (1..256)"); std::process::exit(1); });
        host::run(&by_name, workers);
        return;
    }
    if args.first().is_some_and(|a| a == "--bench-parallel") {
        let result=(||->Result<Value,String>{
            let config:Value=serde_json::from_str(&std::fs::read_to_string(args.get(1).ok_or("need config JSON path")?).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            let paths:Vec<String>=list(&config,"traces").iter().map(|p|p.as_str().map(str::to_owned).ok_or("trace is a path")).collect::<Result<_,_>>()?;
            let mut report=parallel::run(&by_name,&paths,config["workers"].as_u64().unwrap_or(1) as usize,config["copies"].as_u64().unwrap_or(1) as usize)?;
            report["cold_catalog_load_seconds"]=json!(catalog_load_seconds);
            Ok(report)
        })();
        match result {Ok(report)=>println!("{report}"),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}
        return;
    }
    if args.first().is_some_and(|a| a == "--bench-replay") {
        let result = (|| -> Result<Value, String> {
            let path = args.get(1).ok_or("--bench-replay requires a trace path")?;
            let runs = match &args[2..] {
                [] => 3,
                [flag, count] if flag == "--runs" => count.parse().map_err(|_| "invalid --runs")?,
                _ => return Err("usage: game-serve --bench-replay TRACE [--runs N]".into()),
            };
            replay::run(&by_name, path, runs)
        })();
        match result {
            Ok(report) => println!("{report}"),
            Err(error) => { eprintln!("{error}"); std::process::exit(1); }
        }
        return;
    }
    let mut out = std::io::stdout().lock();
    let mut say = |v: Value| {
        #[cfg(feature = "profile")]
        let _writing = game::profile::span(game::profile::Scope::Serialize);
        writeln!(out, "{v}").and_then(|()| out.flush()).expect("the reader is there");
    };
    say(json!({ "type": "ready", "version": version() }));
    let mut game: Option<Simulation> = None;
    let mut session_id = 0u64;
    let mut logged = Delivery::default();
    #[cfg(feature = "profile")]
    let mut profiling = std::env::var_os("MAGIC_PROFILE").map(|path| std::fs::File::create(path).expect("create profile output"));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() { continue; }
        #[cfg(feature = "profile")]
        game::profile::reset();
        let mut replacing = false;
        let answer = (|| -> Result<Value, Error> {
            let request: Value = serde_json::from_str(&line).map_err(|e| format!("not JSON: {e}"))?;
            replacing = matches!(request["cmd"].as_str(), Some("new" | "start"));
            dispatch(&by_name, &mut game, &mut session_id, &mut logged, &request)
        })();
        say(match answer {
            // a player's prompt is said as one, not as the state of the game
            Ok(prompt) if prompt.get("api").is_some() => json!({ "ok": true, "prompt": prompt }),
            Ok(Value::Object(mut said)) if said.contains_key("version") => json!({ "ok": true, "version": said.remove("version") }),
            Ok(Value::Object(mut said)) if said.contains_key("labels") => json!({ "ok": true, "labels": said.remove("labels") }),
            Ok(state) => json!({ "ok": true, "state": state }),
            // A rejected replacement belongs to the candidate, not the retained session.
            Err(error) => match game.as_ref().filter(|_| !replacing).and_then(Simulation::fault) {
                Some(fault) => faulted(session_id, fault),
                None => error.said(),
            },
        });
        #[cfg(feature = "profile")]
        if let Some(file) = &mut profiling {
            writeln!(file, "{}", game::profile::take()).expect("write profile");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn r603_3c_modes_only_wire_answer_retains_empty_and_rejects_extra_content() {
        for indices in [json!([]), json!([0]), json!([2, 0])] {
            let command = json!({"kind":"modes", "modes":indices});
            let action = read_move(&command, &Pool::default()).unwrap();
            assert_eq!(answered(&action), command);
        }
        for (command, reason) in [
            (json!({"kind":"modes"}), "modes is a list"),
            (json!({"kind":"modes","modes":[-1]}), "a mode is a nonnegative index"),
            (json!({"kind":"modes","modes":[0],"targets":[]}), "a modes move has no \"targets\"; it says modes"),
            (json!({"kind":"modes","modes":[0],"division":[]}), "a modes move has no \"division\"; it says modes"),
        ] {
            assert_eq!(read_move(&command, &Pool::default()), Err(reason.into()), "{command}");
        }
    }

    #[test]
    fn r603_3c_d_split_wire_question_exposes_retained_modes_and_only_chosen_slots() {
        let card = CardDef::new("Watcher", "{0}", "Creature",
            "When this creature enters, choose one —\n• You gain 2 life.\n• Target player loses 3 life.", Some((2,2))).unwrap();
        let mut g = Game::new(vec![card], 2, 20);
        for p in 0..2 { for _ in 0..4 { g.put(p,0,Zone::Library); } }
        let card = g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast { card, targets:vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority,Move::Pass).unwrap(); }
        assert_eq!(asked(&mut g)["kind"], "modes");
        g.act(0,read_move(&json!({"kind":"modes","modes":[1]}),&Pool::default()).unwrap()).unwrap();
        let question = asked(&mut g);
        assert_eq!(question["kind"], "targets");
        assert_eq!(question["modes"], json!([1]));
        assert_eq!(question["slots"].as_array().unwrap().len(),1);
        g.act(0,read_move(&json!({"kind":"targets","targets":[[{"player":1}]]}),&Pool::default()).unwrap()).unwrap();
        assert_eq!(g.obj(g.stack[0]).modes,vec![1]);
        assert_eq!(g.obj(g.stack[0]).targets,vec![vec![Target::Player(1)]]);
    }

    #[test]
    fn r603_3c_d_modal_probe_captures_real_boundary_and_leaves_source_unchanged() {
        let card=CardDef::new("Watcher","{0}","Creature",
            "When this creature enters, choose one —\n• You gain 2 life.\n• Target player loses 3 life.",Some((2,2))).unwrap();
        let mut g=Game::new(vec![card],2,20);
        for p in 0..2 { for _ in 0..4 { g.put(p,0,Zone::Library); } }
        let card=g.put(0,0,Zone::Hand);
        g.act(0,Move::Cast { card,targets:vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority,Move::Pass).unwrap(); }
        let (mut host,mut session,mut delivery,faces)=(Some(Simulation::new(g)),1,Delivery::default(),Catalog::new(BTreeMap::new()));
        let before=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"state"})).unwrap();
        let frozen=format!("{:?}",host.as_ref().unwrap().game().unwrap());
        let frozen_delivery=delivery.clone();
        let command=json!({"kind":"targets","modes":[1],"targets":[[{"player":1}]]});
        let request=json!({"cmd":"quote_modal","session_id":1,"request_id":before["request_id"],"player":0,"move":command});
        let proof=dispatch(&faces,&mut host,&mut session,&mut delivery,&request).unwrap();
        assert_eq!(proof["intermediate"]["state"]["asked"]["modes"],json!([1]));
        assert_eq!(proof["intermediate"]["state"]["asked"]["kind"],"targets");
        assert_eq!(proof["intermediate"]["prompt"]["decision"]["kind"],"targets");
        assert_eq!(proof["intermediate"]["prompt"]["request_id"],proof["intermediate"]["state"]["request_id"]);
        assert!(proof["intermediate"]["labels"].as_object().is_some_and(|labels|!labels.is_empty()));
        assert_eq!(proof["equivalent"],true);
        assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),frozen);
        assert_eq!(delivery,frozen_delivery);
        for (changed, class, reason) in [
            (json!({"viewer":0}), Class::Malformed, "quote_modal is privileged"),
            (json!({"session_id":2}), Class::Stale, "stale session_id"),
            (json!({"request_id":999}), Class::Stale, "stale request_id"),
            (json!({"player":1}), Class::Unreferable, "another player's request"),
            (json!({"move":{"kind":"targets","modes":[0],"targets":[]}}), Class::Malformed, "quote_modal needs the exact combined modes and targets command"),
        ] {
            let mut invalid=request.clone();
            invalid.as_object_mut().unwrap().extend(changed.as_object().unwrap().clone());
            assert_eq!(dispatch(&faces,&mut host,&mut session,&mut delivery,&invalid), Err(Error::Refused(class, reason.into())));
            assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),frozen);
            assert_eq!(delivery,frozen_delivery);
            assert_eq!(session,1);
        }
        let final_state=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,
            "request_id":before["request_id"],"player":0,"move":command,"capture_random":true})).unwrap();
        let actual=modal_capture_frame(&faces,&mut host,&mut session,&mut delivery,final_state,None).unwrap();
        assert_eq!(modal_game_facts(actual.clone()),modal_game_facts(proof["final"].clone()));
        let reclaimed=proof["intermediate"]["audit"]["objects"].as_object().unwrap().iter()
            .any(|(id,row)|row["zone"]=="Gone" && actual["audit"]["objects"].get(id).is_none());
        assert!(reclaimed,"the final ordinary response must collect the departed payloads retained only for the probe");
        // 603.3d places the ability on the stack; its target player does not
        // lose life until resolution (608.2c).
        let live=host.as_ref().unwrap().game().unwrap();
        assert_eq!(live.players[1].life,20);
        assert_eq!(live.obj(*live.stack.last().unwrap()).targets,vec![vec![Target::Player(1)]]);
    }

    #[test]
    fn initial_position_priority_steps_install_causes_before_settlement() {
        // CR117.5/704.5g: the complete 5/5 bear survives 3 marked damage.
        // Evaluating the intermediate printed 2/2 would wrongly kill it.
        let faces=corpus::load(&corpus::default_path()).unwrap();
        let catalog=Catalog::new(CardDef::catalog(&faces));
        let mut position=json!({"timing":{"turn":3,"step":"upkeep","active":"A"},"next":{"kind":"priority","chooser":"A"},
            "players":{"A":{"life":18,"mana":{"G":1}},"B":{"life":17,"mana":{}}},
            "zones":{"A.library":["top","bottom"],"B.library":[],"A.hand":["hand"],"B.hand":[],"A.graveyard":["growth"],"B.graveyard":[],
                "battlefield":["bear"],"stack":[],"exile":["exiled"],"command":[]},
            "objects":{
                "bear":{"name":"Grizzly Bears","owner":"B","controller":"A","tapped":true,"counters":{},"damage":3},
                "growth":{"name":"Giant Growth","owner":"A","controller":"A","tapped":false,"counters":{},"damage":0},
                "top":{"name":"Forest","owner":"A","controller":"A","tapped":false,"counters":{},"damage":0},
                "bottom":{"name":"Island","owner":"A","controller":"A","tapped":false,"counters":{},"damage":0},
                "hand":{"name":"Mountain","owner":"A","controller":"A","tapped":false,"counters":{},"damage":0},
                "exiled":{"name":"Swamp","owner":"B","controller":"B","tapped":false,"counters":{},"damage":0}},
            "history":{"turn_structure":"ordinary_A_first","pending":"none","effects":"represented_only","turn_events":"empty","previous_turn_spells":0,
                "land_plays_this_turn":{"A":0,"B":0},"ability_uses":"none","consecutive_passes":0,"control_since":{"bear":"this_turn"}},
            "causal":{"linked_exile":[],"prevention":[],"base_controllers":{"bear":"A"},
                "source_objects":{"old-growth":{"physical":"growth","name":"Giant Growth"}},
                "continuous_effects":[{"id":"boost","producer":{"source":"old-growth","definition":{"oracle":"5748ebf1-24e3-499d-ab7c-c2cebd462a24"},"paragraph":1,"origin":{"kind":"printed"}},
                    "controller":"A","subjects":["bear"],"changes":[{"add_pt":{"power":3,"toughness":3}}],"expires":{"kind":"end_of_turn","turn":3}}]}});
        for (step,part) in [("upkeep",ir::Part::Upkeep),("draw",ir::Part::Draw),("precombat_main",ir::Part::PrecombatMain),
            ("begin_combat",ir::Part::BeginningOfCombat),("end_combat",ir::Part::EndOfCombat),("postcombat_main",ir::Part::PostcombatMain),("end",ir::Part::End)] {
            position["timing"]["step"]=json!(step);
            let game=new_position(&catalog,&position).unwrap();
            assert_eq!(game.part,part);assert_eq!(game.battlefield.len(),1);
            let bear=game.battlefield[0];
            assert_eq!(game.controller(bear),0);assert_eq!(game.get_object(bear).unwrap().owner,1);
            assert!(game.summoning_sick(bear));assert!(game.get_object(bear).unwrap().tapped);
            assert_eq!(game.get_object(bear).unwrap().damage,3);
            assert_eq!((game.players[0].life,game.players[1].life),(18,17));
            assert_eq!(game.players[0].pool.colors[ir::Color::Green as usize],1);
            let library:Vec<_>=game.players[0].library.iter().rev().map(|id|game.get_object(*id).unwrap().chars.name.as_str()).collect();
            assert_eq!(library,vec!["Forest","Island"]);
            assert_eq!(game.players[0].hand.len(),1);assert_eq!(game.exile.len(),1);
            assert!(game.stack.is_empty());assert!(game.log.is_empty());
        }
        position["causal"]["continuous_effects"]=json!([]);
        position["causal"]["base_controllers"]=json!({});
        position["history"]["effects"]=json!("intrinsic_only");
        assert_eq!(new_position(&catalog,&position).unwrap_err(),"initial position is not settled before priority (117.5/704.3)");
    }

    #[test]
    fn initial_activation_selector_authenticates_definition_and_exact_lifetime() {
        let faces=corpus::load(&corpus::default_path()).unwrap();
        let catalog=Catalog::new(CardDef::catalog(&faces));
        let usage=json!({"ability":{"source":"vivi@1","definition":{"oracle":"0452be2a-e97a-4269-a9e3-b616265ecb2e"},
            "paragraph":1,"origin":{"kind":"printed"}},"lifetime":{"kind":"turn","turn":3},"count":1});
        let mut position=json!({"timing":{"turn":3,"step":"precombat_main","active":"A"},"next":{"kind":"priority","chooser":"A"},
            "players":{"A":{"life":20,"mana":{}},"B":{"life":20,"mana":{}}},
            "zones":{"A.library":[],"B.library":[],"A.hand":[],"B.hand":[],"A.graveyard":[],"B.graveyard":[],"battlefield":["vivi@1"],"stack":[],"exile":[],"command":[]},
            "objects":{"vivi@1":{"name":"Vivi Ornitier","owner":"A","controller":"A","tapped":false,"counters":{},"damage":0}},
            "history":{"turn_structure":"ordinary_A_first","pending":"none","effects":"intrinsic_only","turn_events":"empty","previous_turn_spells":0,
                "land_plays_this_turn":{"A":0,"B":0},"ability_uses":"represented_activations_only","consecutive_passes":0,"control_since":{"vivi@1":"before_turn"}},
            "causal":{"linked_exile":[],"prevention":[],"continuous_effects":[],"base_controllers":{},"activation_uses":[usage]}});
        let game=new_position(&catalog,&position).unwrap();
        assert_eq!(game.audit_game_objects()["activation_uses"][0]["count"],1);
        for (mutation, reason) in [
            ("oracle", "initial activation definition differs from its source"),
            ("turn", "initial activation belongs to a different turn"),
            ("count", "initial activation count must be positive"),
            ("missing", "represented activation history requires its complete inventory"),
        ] {
            let mut bad=position.clone();
            match mutation {
                "oracle"=>bad["causal"]["activation_uses"][0]["ability"]["definition"]=json!({"oracle":"not-vivi"}),
                "turn"=>bad["causal"]["activation_uses"][0]["lifetime"]["turn"]=json!(2),
                "count"=>bad["causal"]["activation_uses"][0]["count"]=json!(true),
                _=>{bad["causal"].as_object_mut().unwrap().remove("activation_uses");},
            }
            assert_eq!(new_position(&catalog,&bad).unwrap_err(), reason, "{mutation}");
            assert_eq!(game.audit_game_objects()["activation_uses"][0]["count"],1);
        }
        position["history"]["ability_uses"]=json!("none");
        position["causal"].as_object_mut().unwrap().remove("activation_uses");
        assert_eq!(new_position(&catalog,&position).unwrap().audit_game_objects()["activation_uses"],json!([]),
            "an explicit no-use premise supplies its own empty inventory");
    }

    #[test]
    fn r723_controlled_requests_share_information_but_only_the_controller_can_answer() {
        let control=CardDef::new("Control","{0}","Sorcery","You gain control of target opponent during that player's next turn. After that turn, that player takes an extra turn.",None).unwrap();
        let spell=CardDef::new("Victims spell","{1}","Instant","You gain 2 life.",None).unwrap();
        let mut g=Game::new(vec![control,spell],3,20);
        for p in 0..3 {for _ in 0..5 {g.put(p,1,Zone::Library);}}
        let victim=g.put(1,1,Zone::Hand);let other=g.put(2,1,Zone::Hand);
        let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap();
        while g.turn==1 {
            let mv=if matches!(g.decision,Some(Decision::Attackers{..})){Move::Attack(vec![])}else{Move::Pass};
            g.act(g.decision_maker(g.decision_actor()),mv).unwrap();
        }
        let mana=CardDef::new("Limited mana","","Land","{T}: Add {C}. Spend this mana only to cast an instant spell.",None).unwrap();
        game::play::check(&mana).unwrap();std::sync::Arc::make_mut(&mut g.defs).push(mana);
        let land=g.put(1,2,Zone::Battlefield);g.act(0,Move::Activate{source:land,ability:0,targets:vec![],paying:vec![]}).unwrap();
        assert_eq!(g.players[1].pool.restricted.len(),1);
        let mut simulation=Some(Simulation::new(g));let (mut session,mut logged,faces)=(1,Delivery::default(),Catalog::new(BTreeMap::new()));
        let first=dispatch(&faces,&mut simulation,&mut session,&mut logged,&json!({"cmd":"state","viewer":"asked"})).unwrap();
        assert_eq!(first["asked"]["player"],0);assert_eq!(first["asked"]["acting"],1);assert_eq!(first["players"][1]["decision_maker"],0);
        assert!(first["players"][1]["hand"].as_array().unwrap().contains(&json!(victim.0)));
        assert!(first["objects"].get(other.0.to_string()).is_none());
        let hidden=dispatch(&faces,&mut simulation,&mut session,&mut logged,&json!({"cmd":"state","viewer":2})).unwrap();
        assert_eq!(hidden["asked"]["acting"],1);assert!(hidden["asked"].get("moves").is_none());assert!(hidden["players"][1].get("hand").is_none());
        let mut act=json!({"cmd":"act","session_id":1,"request_id":first["request_id"],"player":1,"viewer":"asked","move":{"kind":"cast","card":victim.0,"targets":[],"mana":{"restricted":[0]}}});
        assert_eq!(dispatch(&faces,&mut simulation,&mut session,&mut logged,&act), Err(Error::unreferable("request belongs to player 0, not 1")));
        let retained=simulation.as_mut().unwrap();
        assert_eq!(retained.request().unwrap().unwrap().id.0, first["request_id"].as_u64().unwrap());
        assert_eq!(retained.game().unwrap().players[1].pool.restricted.len(),1);
        assert_eq!(retained.game().unwrap().players[1].hand,vec![victim]);
        assert!(retained.game().unwrap().stack.is_empty());
        act["player"]=json!(0);
        let result=dispatch(&faces,&mut simulation,&mut session,&mut logged,&act).unwrap();
        assert_eq!(result["asked"]["acting"],1);
        assert_eq!(simulation.as_ref().unwrap().game().unwrap().players[1].pool.total(),0);
        assert_eq!(simulation.as_ref().unwrap().game().unwrap().players[0].pool.colorless,0);
    }
    #[test]
    fn r614_5_replacement_wire_identity_round_trips_a_granted_instance() {
        // Uncited (protocol): fabricated identities test wire authentication, not an applied replacement.
        use game::binding::{AbilityKey, Grant, Origin};
        let def = CardDef::new("Giver", "{0}", "Artifact", "{T}: Add {G}.", None).unwrap();
        let what = game::ability::template(&def.abilities()[0]).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let effect = game::operation::ReplacementId::from_ability(source, AbilityKey { index: 0, origin: Origin::Given { what,
                grant: Grant { by: Some(source), stamp: 3, part: 1 } } }, 0);
        let answer = json!({"kind":"choose", "answer":{"replacement":{
            "source":source.0, "part":0, "ability":replacement_ability(effect.ability().unwrap().clone())}}});
        assert_eq!(read_move_offered(&answer, &Pool::default(), &[effect.clone()]).unwrap(), Move::Choose(Answer::Replacement(effect.clone())));
        assert_eq!(read_move_offered(&answer, &Pool::default(), &[]), Err(Error::unreferable("that replacement identity was not offered")));
        let own = game::operation::ReplacementId::from_ability(source, AbilityKey::printed(0), 0);
        assert_eq!(read_move_offered(&answer, &Pool::default(), &[own]), Err(Error::unreferable("that replacement identity was not offered")));
    }
    #[test]
    fn r707_7_replacement_wire_identity_distinguishes_copy_acquisitions() {
        // Uncited (protocol): each copied acquisition is a distinct offered wire identity.
        use game::binding::{AbilityKey, Origin};
        let def=CardDef::new("Model", "{0}", "Artifact", "", None).unwrap();
        let mut g=Game::new(vec![def],2,20);let source=g.put(0,0,Zone::Battlefield);
        let effect=game::operation::ReplacementId::from_ability(source, AbilityKey { index: 0, origin: Origin::Copied(7) }, 0);
        let answer=json!({"kind":"choose", "answer":{"replacement":{
            "source":source.0, "part":0, "ability":replacement_ability(effect.ability().unwrap().clone())}}});
        assert_eq!(read_move_offered(&answer,&Pool::default(),&[effect.clone()]).unwrap(),Move::Choose(Answer::Replacement(effect.clone())));
        let old=game::operation::ReplacementId::from_ability(source, AbilityKey { origin: Origin::Copied(6), ..effect.ability().unwrap().clone() }, 0);
        assert_eq!(read_move_offered(&answer,&Pool::default(),&[old]), Err(Error::unreferable("that replacement identity was not offered")));
        let own=game::operation::ReplacementId::from_ability(source, AbilityKey::printed(0), 0);
        assert_eq!(read_move_offered(&answer,&Pool::default(),&[own]), Err(Error::unreferable("that replacement identity was not offered")));
    }
    use super::*;

    #[test]
    fn r603_3b_the_pipe_distinguishes_granted_triggers_with_the_same_source_and_index() {
        let defs = vec![
            game::CardDef::new("Bearer", "", "Creature", "", Some((2, 2))).unwrap(),
            game::CardDef::new("First", "{0}", "Sorcery", "Until end of turn, target creature gains \"When this creature dies, you gain 2 life.\"", None).unwrap(),
            game::CardDef::new("Second", "{0}", "Sorcery", "Until end of turn, target creature gains \"When this creature dies, you lose 1 life.\"", None).unwrap(),
            game::CardDef::new("Altar", "", "Artifact", "Sacrifice a creature: Add {C}.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let bearer = g.put(0, 0, Zone::Battlefield);
        let altar = g.put(0, 3, Zone::Battlefield);
        for index in [1, 2] {
            let card = g.put(0, index, Zone::Hand);
            g.act(0, Move::Cast { card, targets: vec![vec![Target::Object(bearer)]] }).unwrap();
            for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        }
        g.act(0, Move::Activate { source: altar, ability: 0, targets: vec![], paying: vec![vec![bearer]] }).unwrap();
        let shown = asked(&mut g);
        assert_eq!(shown["kind"], "order");
        let entries = shown["abilities"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["source"], entries[1]["source"]);
        assert_eq!(entries[0]["ability"], entries[1]["ability"]);
        assert_eq!(entries[0]["origin"], "given");
        assert_ne!(entries[0]["grant"], entries[1]["grant"]);
        assert!(entries[0]["text"].as_str().unwrap().contains("gain 2 life"));
        assert!(entries[1]["text"].as_str().unwrap().contains("lose 1 life"));
        let answer = read_move(&json!({"kind":"order", "order":[1, 0]}), &Pool::default()).unwrap();
        g.act(0, answer).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.players[0].life, 22);
    }

    #[test]
    fn r603_3b_the_order_prompt_shows_the_text_a_keyword_trigger_stands_for() {
        let defs = vec![game::CardDef::new("Crier", "", "Creature — Goblin", "Battle cry\nWhenever this creature attacks, you gain 1 life.", Some((1, 1))).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let crier = g.put(0, 0, Zone::Battlefield);
        while g.part != ir::Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(0, Move::Attack(vec![(crier, Target::Player(1))])).unwrap();
        let shown = asked(&mut g);
        assert_eq!(shown["kind"], "order");
        let texts: Vec<_> = shown["abilities"].as_array().unwrap().iter().map(|e| e["text"].as_str().unwrap_or_else(|| panic!("{e}")).to_string()).collect();
        assert!(texts.iter().any(|t| t.contains("each other attacking creature gets +1/+0")), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("you gain 1 life")), "{texts:?}");
    }

    #[test]
    fn r601_2g_the_pipe_casts_with_the_offered_mana_plan_atomically() {
        let defs = vec![
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            game::CardDef::new("Reward", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
            game::CardDef::new("Watcher", "{0}", "Enchantment", "Whenever a land is tapped for mana, target player loses 1 life.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let land = g.put(0, 0, Zone::Battlefield);
        let card = g.put(0, 1, Zone::Hand);
        g.put(1, 2, Zone::Battlefield);
        let request = json!({"kind": "cast", "card": card.0, "targets": [], "mana":{"G":1}, "plan": [{"source": land.0, "ability": 0, "paying": [], "adding": []}]});
        g.act(0, read_move(&request, &g.players[0].pool).unwrap()).unwrap();
        assert_eq!(g.stack.len(), 1);
        assert_eq!(g.players[0].pool.total(), 0);
        assert!(g.obj(land).tapped);
        assert!(matches!(g.decision, Some(game::state::Decision::Targets { player: 1, .. })));
        assert_eq!(read_move(&json!({"kind": "cast", "card": 0, "plan": "invalid"}), &Pool::default()), Err("a mana plan is a list of activations".into()));
    }

    /// A move or answer that says something its kind doesn't is refused, not read without it:
    /// "face" for "cast_as" would otherwise cast the creature instead of its Adventure.
    /// What a listing shows is still taken back with it.
    #[test]
    fn a_move_with_a_key_its_kind_doesnt_say_is_refused() {
        let refused = read_move(&json!({"kind": "cast", "card": 3, "face": "Stoke Genius", "plan": []}), &Pool::default()).unwrap_err();
        assert!(refused.starts_with("a cast move has no \"face\";") && refused.contains("cast_as"), "{refused}");
        assert_eq!(read_move(&json!({"kind": "pass", "card": 3}), &Pool::default()), Err("a pass move has no \"card\"; it says nothing more".into()));
        assert_eq!(read_move(&json!({"kind": "choose", "answer": {"objects": [], "player": 1}}), &Pool::default()), Err("an answer says one thing, not objects and player".into()));
        let unknown = read_move(&json!({"kind": "choose", "answer": {"object": [3]}}), &Pool::default()).unwrap_err();
        assert!(unknown.starts_with("an answer has no \"object\";"), "{unknown}");
        assert!(read_move(&json!({"kind": "choose", "answer": {"top": [], "away": []}}), &Pool::default()).is_ok());
        assert_eq!(read_move(&json!({"kind":"choose", "answer":{"accept":false}}), &Pool::default()).unwrap(), Move::Choose(Answer::OptionalObjects(None)));
        assert_eq!(read_move(&json!({"kind":"choose", "answer":{"accept":true,"objects":[3]}}), &Pool::default()).unwrap(), Move::Choose(Answer::OptionalObjects(Some(vec![game::ObjId(3)]))));
        for (answer, reason) in [
            (json!({"accept":false,"objects":[]}), "optional objects: decline has no objects"),
            (json!({"accept":true}), "optional objects: acceptance requires objects"),
            (json!({"accept":0}), "optional objects: accept must be a boolean"),
            (json!({"accept":true,"objects":"3"}), "optional objects: acceptance requires objects"),
            (json!({"accept":false,"targets":[]}), "an answer says one thing, not accept and targets"),
        ] {
            assert_eq!(read_move(&json!({"kind":"choose", "answer":answer}), &Pool::default()), Err(reason.into()));
        }
        let listed = json!({"kind": "cast", "card": 3, "cast_as": "adventure", "modes": [], "slots": [], "paying": [], "plan": [], "route": "normal",
            "delve": 0, "convoke": [], "pay_life": 2, "face_down": "morph", "life": [true]});
        assert!(read_move(&listed, &Pool::default()).is_ok());
        let listed = json!({"kind": "activate", "source": 3, "ability": 0, "modes": [], "mana": true, "slots": [], "paying": [], "asks": [], "plan": []});
        assert_eq!(read_move(&listed, &Pool::default()), Err("mana selection is an object".into()), "mana is an exact selection, never the retired mana-ability boolean");
    }

    #[test]
    fn r118_12_the_pipe_asks_and_reads_a_payment_of_an_effect() {
        let defs = vec![
            game::CardDef::new("Swamp", "", "Basic Land — Swamp", "", None).unwrap(),
            game::CardDef::new("Toll", "{0}", "Sorcery", "You may pay {B}. If you do, you gain 3 life.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let swamps = [g.put(0, 0, Zone::Battlefield), g.put(0, 0, Zone::Battlefield)];
        let toll = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast { card: toll, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        let asked = &shown["asked"];
        assert_eq!((asked["kind"].clone(), asked["cost"].clone()), (json!("pay"), json!(["B"])));
        assert!(asked.get("plan").is_none());
        let pool = Pool::default();
        assert_eq!(read_move(&json!({"kind":"choose", "answer":"yes"}), &pool).unwrap(), Move::Choose(Answer::Yes));
        let mut unpaid = g.clone();
        assert_eq!(unpaid.act(0, Move::Choose(Answer::Yes)), Err("submit explicit payment instructions or decline (118.12)".into()));
        assert_eq!(unpaid.players[0].life, 20);
        assert!(swamps.iter().all(|o| !unpaid.obj(*o).tapped));
        assert_eq!(super::asked(&mut unpaid), shown["asked"]);
        // The caller explicitly chooses the second Swamp.
        let other = &swamps[1];
        let said = read_move(&json!({"kind":"choose", "answer":{"pay":{"mana":{"B":1}, "plan":[{"source":other.0, "ability":0}]}}}), &pool).unwrap();
        assert!(matches!(&said, Move::Choose(Answer::Pay { plan, with: Some(with), .. }) if with.colors[2] == 1 && plan.len() == 1 && plan[0].source == *other));
        assert_eq!(answered(&said)["answer"]["pay"]["plan"][0]["source"], json!(other.0));
        // its objects are one group, those tapped for a waterbend cost (701.67a); this cost
        // has none, which the game refuses as the answer is made
        let tapped = read_move(&json!({"kind":"choose", "answer":{"pay":{"plan":[], "paying":[[swamps[0].0]]}}}), &pool).unwrap();
        assert!(matches!(&tapped, Move::Choose(Answer::Pay { tapped, .. }) if *tapped == vec![swamps[0]]));
        assert_eq!(answered(&tapped)["answer"]["pay"]["paying"], json!([[swamps[0].0]]));
        let mut non_waterbend = g.clone();
        assert_eq!(non_waterbend.act(0, tapped), Err("only a waterbend cost is paid by tapping artifacts and creatures (701.67a)".into()));
        assert_eq!(super::asked(&mut non_waterbend), shown["asked"]);
        assert_eq!(non_waterbend.players[0].life, 20);
        assert_eq!(non_waterbend.players[0].pool, pool);
        assert!(swamps.iter().all(|o| !non_waterbend.obj(*o).tapped));
        assert_eq!(read_move(&json!({"kind":"choose", "answer":{"pay":{"plan":[], "paying":[[1], [2]]}}}), &pool), Err("an effect's payment supplies only plan, mana and the objects tapped for a waterbend cost".into()));
        assert_eq!(read_move(&json!({"kind":"choose", "answer":{"pay":"yes"}}), &pool), Err("a payment is an object with plan and mana".into()));
        g.act(0, said).unwrap();
        assert!(g.obj(*other).tapped);
        assert!(!g.obj(swamps[0]).tapped, "only the caller-selected second Swamp pays");
        assert_eq!(g.players[0].pool.total(), 0);
        assert_eq!(g.players[0].life, 23);
    }

    /// The pay ask of a waterbend cost says the cost's generic mana as `waterbend`, as listings
    /// say it, and as `max` how many its payer may tap (701.67a): Ward—Waterbend {4} against
    /// a player with three creatures and a red mana floating is "waterbend 4, up to 3".
    #[test]
    fn r701_67a_the_pay_ask_of_a_waterbend_cost_says_its_generic_mana_and_the_most_tapped() {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let mut defs: Vec<CardDef> = ["The Unagi of Kyoshi Island", "Shock"].map(|n| CardDef::from_faces(catalog.get(n).unwrap()).unwrap()).to_vec();
        defs.push(CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap());
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { g.put(p, 2, Zone::Library); } }
        let unagi = g.put(0, 0, Zone::Battlefield);
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let shock = g.put(1, 1, Zone::Hand);
        let bears: Vec<ObjId> = (0..3).map(|_| g.put(1, 2, Zone::Battlefield)).collect();
        g.act(0, Move::Pass).unwrap();
        g.players[1].pool.colors[3] = 2;
        g.act(1, paid("R", Move::Cast { card: shock, targets: vec![vec![Target::Object(unagi)]] })).unwrap();
        while g.asked().is_none_or(|(_, a)| !matches!(a, Asked::Pay { .. })) { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = asked(&mut g);
        assert_eq!(shown["kind"], json!("pay"), "{shown}");
        let paying = &shown["paying"][0];
        assert_eq!((paying["waterbend"].clone(), paying["min"].clone(), paying["max"].clone()), (json!(4), json!(0), json!(3)), "{shown}");
        assert_eq!(paying["among"], json!(bears.iter().map(|b| b.0).collect::<Vec<_>>()));
    }

    #[test]
    fn r601_2d_the_pipe_says_what_is_divided_and_reads_a_division() {
        let defs = vec![
            game::CardDef::new("Arc", "{0}", "Instant", "Arc deals 3 damage divided as you choose among one, two, or three targets.", None).unwrap(),
            game::CardDef::new("Bears", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            game::CardDef::new("Hellkite", "{0}", "Creature — Dragon", "When this creature enters, it deals 2 damage divided as you choose among one or two targets.", Some((1, 1))).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let bears = g.put(1, 1, Zone::Battlefield);
        let arc = g.put(0, 0, Zone::Hand);
        let pool = Pool::default();
        let cast = |division: Value| json!({"kind":"cast", "card":arc.0, "cast_as":"whole", "modes":[], "plan":[],
            "targets":[[{"object":bears.0}, {"player":1}]], "division":division});
        let mut mismatch=g.clone();
        assert_eq!(mismatch.act(0,read_move(&cast(json!([[2, 1, 0]])),&pool).unwrap()), Err("3 amount(s) for 2 targets: one for each (601.2d)".into()));
        assert_eq!(mismatch.players[0].hand,vec![arc]);
        assert!(mismatch.stack.is_empty());
        assert_eq!((mismatch.obj(bears).damage,mismatch.players[1].life),(0,20));
        assert_eq!(read_move(&cast(json!([2, 1])), &pool), Err("a group's division is a list of amounts (601.2d)".into()));
        g.act(0, read_move(&cast(json!([[2, 1]])), &pool).unwrap()).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert!(!g.battlefield.contains(&bears));
        assert_eq!(g.players[1].life, 19);
        // a triggered ability's targets answer says its division (603.3d)
        let bears = g.put(1, 1, Zone::Battlefield);
        let hellkite = g.put(0, 2, Zone::Hand);
        g.act(0, Move::Cast { card: hellkite, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        assert_eq!((shown["asked"]["kind"].clone(), shown["asked"]["slots"][0]["divide"].clone()), (json!("targets"), json!(2)));
        g.act(0, read_move(&json!({"kind":"targets", "targets":[[{"object":bears.0}, {"player":1}]], "division":[[1, 1]]}), &pool).unwrap()).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!((g.obj(bears).damage, g.players[1].life), (1, 18));
    }

    #[test]
    fn r608_2g_the_pipe_offers_a_cast_as_an_effect_is_applied_and_shows_a_permission() {
        let defs = vec![
            game::CardDef::new("Gift", "{0}", "Sorcery", "Exile the top card of your library. You may cast that card without paying its mana cost.", None).unwrap(),
            game::CardDef::new("Bolt", "{R}", "Instant", "Bolt deals 3 damage to any target.", None).unwrap(),
            game::CardDef::new("Loan", "{0}", "Sorcery", "Exile the top card of your library. You may play it this turn.", None).unwrap(),
            game::CardDef::new("Lease", "{0}", "Sorcery", "Exile the top card of your library. Until the end of your next turn, you may play that card.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        for _ in 0..3 { g.put(0, 1, Zone::Library); }
        let gift = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: gift, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        let asked = &shown["asked"];
        let bolt = g.exile[0];
        assert_eq!((asked["kind"].clone(), asked["cards"].clone()), (json!("cast"), json!([bolt.0])));
        assert!(shown["objects"][bolt.0.to_string()].get("may_play").is_none(), "an offer of the effect, not a lasting permission");
        // 608.2g/118.9: privileged evidence authenticates the effect's actual free
        // permission; the cast request itself does not supply a trusted free flag.
        assert_eq!(g.audit_game_objects()["objects"][bolt.0.to_string()]["offered_casts"],
            json!([{"player":0,"cast_as":"whole","without_paying":true,"during_resolution":true}]));
        let mut declined=g.clone();
        declined.act(0,Move::Choose(Answer::No)).unwrap();
        assert_eq!(declined.audit_game_objects()["objects"][bolt.0.to_string()]["offered_casts"],json!([]));
        let pool = Pool::default();
        let said = read_move(&json!({"kind":"cast", "card":bolt.0, "cast_as":"whole", "modes":[], "plan":[], "targets":[[{"player":1}]]}), &pool).unwrap();
        g.act(0, said).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.players[1].life, 17);

        let loan = g.put(0, 2, Zone::Hand);
        g.act(0, Move::Cast { card: loan, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        assert_eq!(shown["asked"]["kind"], json!("priority"));
        assert_eq!(shown["objects"][g.exile[0].0.to_string()]["may_play"], json!({"player": 0, "land": true, "until": "end_of_turn"}));
        assert_eq!(g.audit_game_objects()["objects"][g.exile[0].0.to_string()]["offered_casts"],json!([]));
        // until when, said of the player it is given to (611.2a)
        let lease = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card: lease, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["objects"][g.exile[1].0.to_string()]["may_play"], json!({"player": 0, "land": true, "until": "end_of_their_next_turn"}));
    }

    #[test]
    fn r107_4f_the_pipe_reads_life_paid_for_phyrexian_mana() {
        let defs = vec![game::CardDef::new("Probe", "{U/P}", "Sorcery", "You gain 1 life.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        let pool = Pool::default();
        let request = json!({"kind":"cast", "card":card.0, "targets":[], "plan":[], "life":[true]});
        let said = read_move(&request, &pool).unwrap();
        assert!(matches!(&said, Move::WithMana(_, inner) if matches!(&**inner, Move::WithLife(life, _) if life == &vec![true])) || matches!(&said, Move::WithLife(..)), "{said:?}");
        assert_eq!(read_move(&json!({"kind":"cast", "card":card.0, "targets":[], "life":[1]}), &pool), Err("life is a list of true or false, one for each Phyrexian symbol".into()));
        assert_eq!(read_move(&json!({"kind":"pass", "life":[true]}), &pool), Err("a pass move has no \"life\"; it says nothing more".into()));
        let mut unsaid = request.clone(); unsaid.as_object_mut().unwrap().remove("life");
        let mut unpaid = g.clone();
        assert_eq!(unpaid.act(0, read_move(&unsaid, &pool).unwrap()), Err("your mana pool cannot pay that cost (118.3a): {U}".into()));
        assert_eq!(unpaid.players[0].life, 20);
        assert_eq!(unpaid.players[0].hand, vec![card]);
        assert!(unpaid.stack.is_empty());
        g.act(0, said).unwrap();
        assert_eq!(g.players[0].life, 18);
    }

    #[test]
    fn r107_3a_the_pipe_reads_and_shows_an_announced_x() {
        let defs = vec![
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            game::CardDef::new("Reward", "{X}{G}", "Instant", "You gain X life.", None).unwrap(),
            game::CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let lands: Vec<_> = (0..3).map(|_| g.put(0, 0, Zone::Battlefield)).collect();
        let card = g.put(0, 1, Zone::Hand);
        let _bear = g.put(0, 2, Zone::Hand);
        let plan: Vec<_> = lands.iter().map(|o| json!({"source":o.0,"ability":0,"paying":[],"adding":[]})).collect();
        let request = json!({"kind":"cast", "card":card.0,"targets":[],"plan":plan,"x":2,"mana":{"G":3}});
        let before = format!("{g:?}");
        let mut missing = request.clone(); missing.as_object_mut().unwrap().remove("x");
        assert_eq!(g.act(0, read_move(&missing, &Pool::default()).unwrap()), Err("that cost has {X}: announce the value of X (107.3a, 601.2b)".into()));
        assert_eq!(format!("{g:?}"), before);
        let mut unpaid = request.clone(); unpaid["x"] = json!(3);
        assert_eq!(g.act(0, read_move(&unpaid, &Pool::default()).unwrap()), Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into()));
        assert_eq!(format!("{g:?}"), before);
        for bad in [json!(-1), json!(1.5), json!("2")] {
            let mut request = request.clone(); request["x"] = bad;
            assert_eq!(read_move(&request, &Pool::default()), Err("x is a whole number that is not negative (107.1b)".into()));
        }
        assert_eq!(read_move(&json!({"kind":"pass", "x":1}), &Pool::default()), Err("a pass move has no \"x\"; it says nothing more".into()));
        let relock = read_move(&json!({"kind":"payment", "plan":[], "x":1}), &Pool::default()).unwrap_err();
        assert!(relock.starts_with("a payment move has no \"x\";"), "a payment correction cannot change the locked X (601.2f): {relock}");
        g.act(0, read_move(&request, &Pool::default()).unwrap()).unwrap();
        let spell = g.stack[0];
        for viewer in [Some(0), Some(1), None] {
            let shown = state(&mut g, viewer, &mut Delivery::default());
            let object = &shown["objects"][spell.0.to_string()];
            assert_eq!(object["x"], json!(2), "{object}");
        }
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.players[0].life, 22);
    }

    #[test]
    fn r601_2h_the_pipe_offers_and_pays_additional_spell_costs_atomically() {
        let defs = vec![
            game::CardDef::new("Offering", "{G}", "Instant", "As an additional cost to cast this spell, sacrifice a creature.\nYou gain 3 life.", None).unwrap(),
            game::CardDef::new("Elf", "{G}", "Creature — Elf", "{T}: Add {G}.", Some((1, 1))).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let source = g.put(0, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        let request = json!({"kind":"cast", "card":card.0, "targets":[], "paying":[[source.0]], "mana":{"G":1},
            "plan":[{"source":source.0,"ability":0,"paying":[],"adding":[]}]});
        let before = format!("{g:?}");
        let mut bad = request.clone(); bad["paying"] = json!([[card.0]]);
        assert_eq!(g.act(0, read_move(&bad, &Pool::default()).unwrap()), Err("Offering is not what that cost asks for or cannot pay it (701.21a, 118.3)".into()));
        assert_eq!(format!("{g:?}"), before);
        g.act(0, read_move(&request, &Pool::default()).unwrap()).unwrap();
        assert_eq!(g.stack.len(), 1);
        assert_eq!(g.obj(source).zone, Zone::Gone);
        assert_eq!(g.players[0].pool.total(), 0);
        assert_eq!(read_move(&json!({"kind":"cast", "card":0, "paying":"invalid"}), &Pool::default()), Err("paying is a list of payment groups".into()));
    }

    fn simultaneous_choices(text: &str, zone: Zone, counts: &[usize]) -> (Game, Vec<ObjId>) {
        let defs = vec![
            game::CardDef::new("Choices", "{0}", "Instant", text, None).unwrap(),
            game::CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            game::CardDef::new("Wolf", "{0}", "Creature — Wolf", "", Some((3, 3))).unwrap(),
        ];
        let mut g = Game::new(defs, counts.len(), 20);
        let groups: Vec<Vec<_>> = counts.iter().enumerate().map(|(player, count)| (0..*count).map(|i| g.put(player, 1 + i % 2, zone)).collect()).collect();
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        for _ in 0..counts.len() { g.act(g.priority, Move::Pass).unwrap(); }
        (g, groups[0].clone())
    }

    /// Synthetic Scales/Evolution fixtures isolate counter replacement ordering through the pipe.
    #[test]
    fn r616_1_the_pipe_asks_which_replacement_applies_and_takes_the_answer() {
        let defs = vec![
            game::CardDef::new("Hardened Scales", "{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
            game::CardDef::new("Branching Evolution", "{2}{G}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
            game::CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            game::CardDef::new("Rain", "{0}", "Instant", "Put a +1/+1 counter on each creature.", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let mut sides = Vec::new();
        for player in 0..2 {
            sides.push((g.put(player, 0, Zone::Battlefield), g.put(player, 1, Zone::Battlefield), g.put(player, 2, Zone::Battlefield)));
        }
        let spell = g.put(0, 3, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let (scales, evolution, bear) = sides[0];
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        assert_eq!(shown["asked"], json!({
            "player": 0, "kind": "replacement", "event": "put_counters", "on": {"object": bear.0}, "optional": false, "counter": "+1/+1", "count": 1, "counters": [{"counter":"+1/+1","count":1}], "by":0,
            "among": [
                { "source": scales.0, "ability": 0, "part": 0, "name": "Hardened Scales" },
                { "source": evolution.0, "ability": 0, "part": 0, "name": "Branching Evolution" },
            ],
        }));
        // an effect that is not offered is refused and the question stands
        let wrong = read_move(&json!({"kind":"choose", "answer":{"replacement":{"source": bear.0, "ability": 0, "part": 0}}}), &Pool::default()).unwrap();
        let retained=format!("{g:?}");
        assert_eq!(g.act(0, wrong), Err("choose one of the replacement effects that apply (616.1)".into()));
        assert_eq!(format!("{g:?}"),retained);
        assert_eq!(state(&mut g, Some(0), &mut Delivery::default())["asked"], shown["asked"]);
        let first = read_move(&json!({"kind":"choose", "answer":{"replacement":{"source": scales.0, "ability": 0, "part": 0}}}), &Pool::default()).unwrap();
        g.act(0, first).unwrap();
        // the other player is asked next, knows the first answer, and nothing is put yet
        let (_, evolution, theirs) = sides[1];
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["player"], 1);
        assert_eq!(shown["asked"]["on"], json!({"object": theirs.0}));
        assert_eq!(shown["choices_made"], json!([{ "player": 0, "on": {"object": bear.0}, "replacement": { "source": sides[0].0.0, "ability": 0, "part": 0 } }]));
        assert_eq!(shown["objects"][bear.0.to_string()]["counters"], json!({}));
        let second = read_move(&json!({"kind":"choose", "answer":{"replacement":{"source": evolution.0, "ability": 0, "part": 0}}}), &Pool::default()).unwrap();
        g.act(1, second).unwrap();
        let shown = state(&mut g, None, &mut Delivery::default());
        assert_eq!(shown["objects"][bear.0.to_string()]["counters"], json!({ "+1/+1": 4 }));
        assert_eq!(shown["objects"][theirs.0.to_string()]["counters"], json!({ "+1/+1": 3 }));
        assert_eq!(shown["choices_made"], json!([]));
        assert_eq!(shown["asked"]["kind"], "priority");
    }

    #[test]
    fn r101_4b_the_next_player_sees_the_previous_players_sacrifice_choice_before_choosing() {
        let (mut g, mine) = simultaneous_choices("Each player sacrifices a creature.", Zone::Battlefield, &[2, 2]);
        g.act(0, Move::Choose(Answer::Objects(vec![mine[0]]))).unwrap();
        assert_eq!(g.battlefield.len(), 4, "the sacrifices have not happened yet");
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["player"], 1);
        assert_eq!(shown["choices_made"], json!([{ "player": 0, "verb": "Sacrifice", "objects": [mine[0].0] }]));
    }

    #[test]
    fn r101_4a_the_next_player_does_not_see_which_hidden_card_was_chosen_for_discard() {
        let (mut g, mine) = simultaneous_choices("Each player discards a card.", Zone::Hand, &[2, 2]);
        let mut different = g.clone();
        g.act(0, Move::Choose(Answer::Objects(vec![mine[0]]))).unwrap();
        different.act(0, Move::Choose(Answer::Objects(vec![mine[1]]))).unwrap();
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        let alternative = state(&mut different, Some(1), &mut Delivery::default());
        assert_eq!(shown, alternative, "the hidden choice cannot affect the opponent's current view");
    }

    #[test]
    fn r101_4b_forced_sacrifice_choices_are_visible_to_later_choosers() {
        let (mut g, mine) = simultaneous_choices("Each player sacrifices a creature.", Zone::Battlefield, &[1, 2]);
        assert_eq!(g.battlefield.len(), 3);
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["player"], 1);
        assert_eq!(shown["choices_made"], json!([{ "player": 0, "verb": "Sacrifice", "objects": [mine[0].0] }]));
    }

    #[test]
    fn r101_4b_cloning_and_resuming_multiplayer_choices_does_not_duplicate_declarations() {
        let (mut g, mine) = simultaneous_choices("Each player sacrifices a creature.", Zone::Battlefield, &[2, 2, 2]);
        g.act(0, Move::Choose(Answer::Objects(vec![mine[0]]))).unwrap();
        let mut clone = g.clone();
        let second = *g.battlefield.iter().find(|o| g.controller(**o) == 1).unwrap();
        for game in [&mut g, &mut clone] { game.act(1, Move::Choose(Answer::Objects(vec![second]))).unwrap(); }
        let shown = state(&mut g, Some(2), &mut Delivery::default());
        assert_eq!(shown, state(&mut clone, Some(2), &mut Delivery::default()));
        assert_eq!(shown["choices_made"], json!([
            { "player": 0, "verb": "Sacrifice", "objects": [mine[0].0] },
            { "player": 1, "verb": "Sacrifice", "objects": [second.0] },
        ]));
        let third = *g.battlefield.iter().find(|o| g.controller(**o) == 2).unwrap();
        g.act(2, Move::Choose(Answer::Objects(vec![third]))).unwrap();
        assert_eq!(g.log.iter().filter(|e| matches!(e, Event::Sacrificed { .. })).count(), 3);
        assert_eq!(state(&mut g, Some(2), &mut Delivery::default())["choices_made"], json!([]));
    }

    #[test]
    fn r101_4b_the_pipe_identifies_entry_questions_and_publishes_prior_entry_answers() {
        let defs = vec![
            game::CardDef::new("Open the Vaults", "{4}{W}{W}", "Sorcery", "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control.", None).unwrap(),
            game::CardDef::new("Prism Ring", "{1}", "Artifact", "As this artifact enters, choose a color.\nWhenever you cast a spell of the chosen color, you gain 1 life.", None).unwrap(),
        ];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = Game::new(defs, 3, 20);
        let first = g.put(0, 1, Zone::Graveyard);
        let second = g.put(1, 1, Zone::Graveyard);
        g.players[0].pool.colors = [6; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, paid("WWWWWW", Move::Cast { card: spell, targets: vec![] })).unwrap();
        for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
        let shown = state(&mut g, Some(0), &mut Delivery::default());
        assert_eq!(shown["asked"]["entering"], first.0);
        let answer = json!({"kind":"choose", "answer":{"value":{"color":"red"}}});
        g.act(0, read_move(&answer, &Pool::default()).unwrap()).unwrap();
        let next = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(next["asked"]["entering"], second.0);
        let previous = json!([{ "player": 0, "entering": first.0, "answer": {"value":{"color":"red"}} }]);
        assert_eq!(next["choices_made"], previous);
        assert_eq!(state(&mut g, Some(2), &mut Delivery::default())["choices_made"], previous);
        assert_eq!(g.obj(first).zone, Zone::Graveyard);
        let answer = json!({"kind":"choose", "answer":{"value":{"color":"blue"}}});
        g.act(1, read_move(&answer, &Pool::default()).unwrap()).unwrap();
        assert_eq!(state(&mut g, Some(2), &mut Delivery::default())["choices_made"], json!([]));
    }

    #[test]
    fn r101_4b_a_public_aura_attachment_is_visible_before_later_entry_choices() {
        let defs = vec![
            game::CardDef::new("Open the Vaults", "{4}{W}{W}", "Sorcery", "Return all artifact and enchantment cards from all graveyards to the battlefield under their owners' control.", None).unwrap(),
            game::CardDef::new("Prism Ring", "{1}", "Artifact", "As this artifact enters, choose a color.\nWhenever you cast a spell of the chosen color, you gain 1 life.", None).unwrap(),
            game::CardDef::new("Pacifism", "{1}{W}", "Enchantment — Aura", "Enchant creature\nEnchanted creature can't attack or block.", None).unwrap(),
            game::CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        ];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let aura = g.put(0, 2, Zone::Graveyard);
        let ring = g.put(1, 1, Zone::Graveyard);
        let bear = g.put(1, 3, Zone::Battlefield);
        g.players[0].pool.colors = [6; 5];
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, paid("WWWWWW", Move::Cast { card: spell, targets: vec![] })).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.entering_card(), Some(aura));
        g.act(0, Move::Choose(Answer::Attach(Target::Object(bear)))).unwrap();
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["entering"], ring.0);
        assert_eq!(shown["choices_made"], json!([{
            "player": 0, "entering": aura.0, "answer": { "attach": target(&Target::Object(bear)) }
        }]));
        assert_eq!(g.obj(aura).zone, Zone::Graveyard);
        g.act(1, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
        assert_eq!(g.battlefield.len(), 3);
    }

    #[test]
    fn r101_4b_entry_copy_selection_is_published_before_the_copied_color_choice() {
        let defs = vec![
            game::CardDef::new("Clone", "{0}", "Creature — Shapeshifter", "You may have this creature enter as a copy of any creature on the battlefield.", Some((0, 0))).unwrap(),
            game::CardDef::new("Color chooser", "{0}", "Creature — Bear", "As this creature enters, choose a color.", Some((2, 2))).unwrap(),
        ];
        for def in &defs { game::play::check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let model = g.put(1, 1, Zone::Battlefield);
        let clone = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: clone, targets: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let entering = g.entering_card().unwrap();
        g.act(0, Move::Choose(Answer::Objects(vec![model]))).unwrap();
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["choices_made"], json!([{
            "player": 0, "entering": entering.0, "answer": { "objects": [model.0] }
        }]));
        let mut copy = g.clone();
        for g in [&mut g, &mut copy] {
            g.act(0, Move::Choose(Answer::Value(game::binding::Value::Color(ir::Color::Red)))).unwrap();
            assert_eq!(state(g, Some(1), &mut Delivery::default())["choices_made"], json!([]));
        }
    }

    #[test]
    fn r401_4_private_library_order_is_not_published_as_a_public_choice() {
        let (mut g, mine) = simultaneous_choices("Put all creatures on top of their owners' libraries.", Zone::Battlefield, &[2, 2]);
        let mut different = g.clone();
        g.act(0, Move::Choose(Answer::Objects(mine.clone()))).unwrap();
        different.act(0, Move::Choose(Answer::Objects(mine.into_iter().rev().collect()))).unwrap();
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["choices_made"], json!([]));
        assert_eq!(shown, state(&mut different, Some(1), &mut Delivery::default()));
    }

    #[test]
    fn r701_23a_the_pipe_keeps_search_choices_and_found_cards_private() {
        let defs = vec![
            game::CardDef::new("Seek", "{0}", "Instant", "Search your library for a card. You may put that card into your hand.", None).unwrap(),
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let found = g.put(0, 1, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
        let key = found.0.to_string();
        let mut logged = Delivery::default();
        let own = state(&mut g, Some(0), &mut logged);
        let opponent = state(&mut g, Some(1), &mut logged);
        assert_eq!(own["asked"]["kind"], "search");
        assert_eq!(own["objects"][&key]["name"], "Forest");
        assert_eq!(opponent["asked"], json!({ "kind": "search", "player": 0 }));
        assert!(opponent["objects"].get(&key).is_none());
        g.act(0, Move::Choose(Answer::Objects(vec![found]))).unwrap();
        let own = state(&mut g, Some(0), &mut logged);
        let opponent = state(&mut g, Some(1), &mut logged);
        assert_eq!(own["asked"]["kind"], "may");
        assert_eq!(own["objects"][&key]["name"], "Forest");
        assert!(opponent["objects"].get(&key).is_none());
    }
}
    #[test]
    fn r702_6a_the_pipe_orders_activated_keywords_with_other_printed_abilities() {
        let def = game::CardDef::new("Harness", "{0}", "Artifact — Equipment",
            "Equip {1}\n{T}: You gain 1 life.\nEquip {2}", None).unwrap();
        game::play::check(&def).unwrap();
        let mut g = Game::new(vec![def], 2, 20);
        let source = g.put(0, 0, Zone::Battlefield);
        let view = state(&mut g, Some(0), &mut Delivery::default());
        assert_eq!(view["objects"][source.0.to_string()]["activated"], json!([3, 1, 4]));
    }

#[test]
fn r111_6_the_pipe_counts_cards_not_tokens_during_a_paused_resolution() {
    for (zone, text, field) in [
        (Zone::Hand, "Return target creature to its owner's hand. You may gain 1 life.", "hand_size"),
        (Zone::Library, "Put target creature on top of its owner's library. You may gain 1 life.", "library"),
    ] {
        let defs = vec![
            game::CardDef::new("Maker", "{0}", "Instant", "Create a 2/2 green Bear creature token.", None).unwrap(),
            game::CardDef::new("Pause", "{0}", "Instant", text, None).unwrap(),
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        for d in &defs { game::play::check(d).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let maker = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast {card:maker,targets:vec![]}).unwrap();
        g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
        let token = g.battlefield[0];
        g.put(0, 2, zone);
        let pause = g.put(0, 1, Zone::Hand);
        g.act(0, Move::Cast {card:pause,targets:vec![vec![Target::Object(token)]]}).unwrap();
        g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
        assert!(g.resolving().is_some());
        for viewer in [Some(0), Some(1), None] {
            let view = state(&mut g, viewer, &mut Delivery::default());
            assert_eq!(view["players"][0][field], 1);
            if zone == Zone::Hand && viewer != Some(1) {
                let hand = view["players"][0]["hand"].as_array().unwrap();
                assert_eq!(hand.len(), 2, "the token remains until the next state-based check");
                let cards = hand.iter().filter(|id| view["objects"][id.to_string()]["is_card"] == true).count();
                assert_eq!(cards, 1, "the hand count includes cards, not every object");
                assert!(hand.iter().any(|id| view["objects"][id.to_string()]["token"] == true
                    && view["objects"][id.to_string()]["is_card"] == false));
            }
        }
    }
}

#[test]
fn r707_10_a_returned_spell_copy_is_neither_a_card_nor_a_token_in_a_paused_hand() {
    let defs = vec![
        game::CardDef::new("Copy", "{0}", "Instant", "Exile the top card of your library. Choose a card exiled this way and copy it. You may cast the copy without paying its mana cost.", None).unwrap(),
        game::CardDef::new("Model", "{0}", "Sorcery", "You gain 1 life.", None).unwrap(),
        game::CardDef::new("Recall", "{0}", "Instant", "Return target spell to its owner's hand. You may gain 1 life.", None).unwrap(),
        game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    for d in &defs { game::play::check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 1, Zone::Library);
    let maker = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card: maker, targets: vec![] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    let Some((0, Asked::Cast { cards, .. })) = g.asked() else { panic!("offer to cast the copy") };
    g.act(0, Move::Cast { card: cards[0], targets: vec![] }).unwrap();
    let copy = *g.stack.last().unwrap();
    g.put(0, 3, Zone::Hand);
    let recall = g.put(0, 2, Zone::Hand);
    g.act(0, Move::Cast { card: recall, targets: vec![vec![Target::Object(copy)]] }).unwrap();
    g.act(0, Move::Pass).unwrap(); g.act(1, Move::Pass).unwrap();
    assert!(g.resolving().is_some());
    for viewer in [Some(0), Some(1), None] {
        let view = state(&mut g, viewer, &mut Delivery::default());
        assert_eq!(view["players"][0]["hand_size"], 1);
        if viewer == Some(1) { continue; }
        let hand = view["players"][0]["hand"].as_array().unwrap();
        assert_eq!(hand.len(), 2);
        assert_eq!(hand.iter().filter(|id| view["objects"][id.to_string()]["is_card"] == true).count(), 1);
        assert!(hand.iter().any(|id| view["objects"][id.to_string()]["name"] == "Model"
            && view["objects"][id.to_string()]["is_card"] == false
            && view["objects"][id.to_string()]["token"] == false));
    }
}

#[test]
fn r601_2c_the_pipe_enforces_supplied_relational_target_groups() {
    let d=game::CardDef::new("Arc Trail","{1}{R}","Sorcery","Arc Trail deals 2 damage to any target and 1 damage to any other target.",None).unwrap();
    let mut g=Game::new(vec![d],3,20);
    for _ in 0..2 {g.add_mana(0,ir::mana::ManaSymbol::Color(ir::Color::Red));}
    let card=g.put(0,0,Zone::Hand);
    let bad=json!({"kind":"cast","card":card.0,"mana":{"R":2},"targets":[[{"player":1}],[{"player":1}]]});
    let before=format!("{g:?}");
    assert_eq!(g.act(0,read_move(&bad,&Pool::default()).unwrap()), Err("another target must differ from the preceding targets (115.1, 601.2c)".into()));
    assert_eq!(format!("{g:?}"),before);
    let good=json!({"kind":"cast","card":card.0,"mana":{"R":2},"targets":[[{"player":1}],[{"player":0}]]});
    g.act(0,read_move(&good,&Pool::default()).unwrap()).unwrap();
}

/// Typed game values at the protocol boundary; entry choices currently use the two scalar
/// kinds. The other variants also describe frame values without inventing a separate type.
fn bound_value(value: &game::binding::Value) -> Value {
    use game::binding::Value as Bound;
    match value {
        Bound::CardType(t) => json!({ "card_type": t }),
        Bound::CardName(t) => json!({ "card_name": t }),
        Bound::CreatureType(t) => json!({ "creature_type": t.name() }),
        Bound::Color(c) => json!({ "color": c.word() }),
        Bound::BasicLandType(t) => json!({ "basic_land_type": t.name() }),
        Bound::Consent(b) => json!({ "consent": b }),
        Bound::Number(n) => json!({ "number": n }),
        Bound::Cards(cards) => json!({ "cards": ids(cards) }),
        Bound::Coin(_) | Bound::Turn(_) | Bound::CreatedTurns(_) => unreachable!("coin and turn results are internal, never player choices"),
        Bound::Destructions(_) | Bound::Removals(_) | Bound::Damage(_) => unreachable!("operation-local results are not public scalar choices"),
        Bound::Subjects(subjects) => json!({ "subjects": subjects.iter().map(target).collect::<Vec<_>>() }),
    }
}
fn read_bound_value(value: &Value) -> Result<game::binding::Value, String> {
    use game::binding::Value as Bound;
    let fields = value.as_object().ok_or("a typed value is an object")?;
    if fields.len() != 1 { return Err("a typed value has exactly one kind".into()); }
    if let Some(number) = value.get("number") {
        return number.as_u64().and_then(|n| u32::try_from(n).ok()).map(Bound::Number)
            .ok_or_else(|| "number must be a nonnegative u32 integer".into());
    }
    if let Some(name) = value.get("card_name").and_then(Value::as_str) {
        let answer = Bound::CardName(name.to_owned());
        return game::binding::accepts(game::binding::Kind::CardName, &answer).then_some(answer).ok_or_else(|| format!("{name} is no card name (201.4)"));
    }
    if let Some(name) = value.get("card_type").and_then(Value::as_str) {
        let answer = Bound::CardType(name.to_owned());
        return game::binding::accepts(game::binding::Kind::CardType, &answer).then_some(answer).ok_or_else(|| format!("{name} is no card type (205.2a)"));
    }
    if let Some(name) = value.get("creature_type").and_then(Value::as_str) {
        return ir::Subtype::from_name(name).map(Bound::CreatureType).ok_or_else(|| format!("{name} is no subtype"));
    }
    if let Some(name) = value.get("color").and_then(Value::as_str) {
        return ir::Color::from_word(name).map(Bound::Color).ok_or_else(|| format!("{name} is no color (105.1)"));
    }
    if let Some(name) = value.get("basic_land_type").and_then(Value::as_str) {
        let answer = ir::Subtype::from_name(name).map(Bound::BasicLandType).ok_or_else(|| format!("{name} is no subtype"))?;
        return game::binding::accepts(game::binding::Kind::BasicLandType, &answer).then_some(answer).ok_or_else(|| format!("{name} is no basic land type (305.6)"));
    }
    Err("choose a card_name, card_type, creature_type, color or basic_land_type value".into())
}

#[cfg(test)]
mod value_choice_tests {
    use super::*;
    #[test]
    fn bounded_number_answers_are_strict_and_use_the_existing_value_wire() {
        // Uncited (protocol): JSON numbers must preserve the unsigned 32-bit value exactly.
        for n in [0, 3, u32::MAX] {
            let value = game::binding::Value::Number(n);
            assert_eq!(read_bound_value(&bound_value(&value)).unwrap(), value);
        }
        for bad in [json!({"number":-1}), json!({"number":1.5}), json!({"number":"3"}),
            json!({"number":4294967296_u64}), json!({"number":null})] {
            assert_eq!(read_bound_value(&bad), Err("number must be a nonnegative u32 integer".into()), "{bad}");
        }
        assert_eq!(read_bound_value(&json!({"number":1,"color":"blue"})), Err("a typed value has exactly one kind".into()));
        let g = Game::new(vec![], 2, 20);
        let rendered = render_asked(&g, Some((0, Asked::Number { min: 0, max: 3 })));
        assert_eq!((rendered["kind"].clone(), rendered["min"].clone(), rendered["max"].clone()), (json!("number"), json!(0), json!(3)));
    }

    #[test]
    fn r608_2d_card_type_values_round_trip_and_invalid_kinds_are_rejected() {
        for value in game::binding::options(game::binding::Kind::CardType) {
            assert_eq!(read_bound_value(&bound_value(&value)).unwrap(), value);
        }
        for (invalid, reason) in [
            (json!({"card_type":"legendary"}), "legendary is no card type (205.2a)"),
            (json!({"card_type":"creature", "color":"red"}), "a typed value has exactly one kind"),
            (json!({"card_name":"not a real card"}), "not a real card is no card name (201.4)"),
            (json!({"card_name":"Forest", "color":"green"}), "a typed value has exactly one kind"),
        ] {
            assert_eq!(read_bound_value(&invalid), Err(reason.into()));
        }
    }

    #[test]
    fn r305_6_basic_land_type_values_round_trip_and_others_are_refused() {
        let options = game::binding::options(game::binding::Kind::BasicLandType);
        assert_eq!(options.len(), 5);
        for value in options {
            assert_eq!(read_bound_value(&bound_value(&value)).unwrap(), value);
        }
        assert_eq!(bound_value(&game::binding::Value::BasicLandType(ir::Subtype::from_name("Island").unwrap())), json!({"basic_land_type":"Island"}));
        for (name, reason) in [("Desert", "Desert is no basic land type (305.6)"), ("Goblin", "Goblin is no basic land type (305.6)"), ("Wastes", "Wastes is no subtype")] {
            assert_eq!(read_bound_value(&json!({"basic_land_type":name})), Err(reason.into()));
        }
    }

    #[test]
    fn r614_12a_typed_choices_round_trip_and_expose_the_producer_and_chooser() {
        for (text, value) in [("color", json!({"color":"red"})), ("creature type", json!({"creature_type":"Goblin"})), ("card name", json!({"card_name":"Black Lotus"}))] {
            let d = CardDef::new("Witness", "", "Land", &format!("As this land enters, choose a {text}."), None).unwrap();
            check(&d).unwrap();
            let mut g = Game::new(vec![d], 3, 20);
            let card = g.put(0, 0, Zone::Hand);
            g.act(0, Move::PlayLand(card)).unwrap();
            let request = asked(&mut g);
            assert_eq!(request["kind"], "value");
            assert_eq!(request["player"], 0);
            assert!(g.battlefield.is_empty(), "614.12a: the as-enters choice precedes entry");
            assert_eq!(g.players[0].hand, vec![card]);
            assert!(request["options"].as_array().unwrap().contains(&value));
            let wrong = read_move(&json!({"kind":"choose", "answer":{"value":{"color":"colorless"}}}), &Pool::default());
            assert_eq!(wrong, Err("colorless is no color (105.1)".into()));
            assert_eq!(read_bound_value(&json!({"color":"red", "creature_type":"Goblin"})), Err("a typed value has exactly one kind".into()));
            let action = read_move(&json!({"kind":"choose", "answer":{"value":value}}), &Pool::default()).unwrap();
            g.act(0, action).unwrap();
            let source = g.battlefield[0];
            for viewer in 0..3 {
                let view = state(&mut g, Some(viewer), &mut Delivery::default());
                assert_eq!(view["objects"][source.0.to_string()]["choices"], json!([{"group":"own", "producer":0, "chooser":0, "value":value}]));
            }
        }
    }
}

#[test]
fn r616_1_the_pipe_describes_and_resumes_entry_order_without_inventing_a_counter_kind() {
    let defs = vec![
        game::CardDef::new("Root Maze", "{G}", "Enchantment", "Artifacts and lands enter tapped.", None).unwrap(),
        game::CardDef::new("Azorius Guildgate", "", "Land — Gate", "This land enters tapped.\n{T}: Add {W} or {U}.", None).unwrap(),
    ];
    for def in &defs { game::play::check(def).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let maze=g.put(0, 0, Zone::Battlefield);
    let land = g.put(0, 1, Zone::Hand);
    g.act(0, Move::PlayLand(land)).unwrap();
    let shown = state(&mut g, Some(0), &mut Delivery::default());
    let asked = &shown["asked"];
    assert_eq!(asked["kind"], "replacement");
    assert_eq!(asked["event"], "entry");
    assert_eq!(asked["counter"], Value::Null);
    assert_eq!(asked["entering"], land.0);
    let retained=format!("{g:?}");
    assert_eq!(g.act(0, Move::Pass), Err("a decision is pending: a choice of how the permanent enters the battlefield (614.12)".into()));
    assert_eq!(format!("{g:?}"),retained);
    let authored=json!({"source":maze.0,"ability":0,"part":0,"name":"Root Maze"});
    assert_eq!(asked["player"],0);
    assert_eq!(asked["among"].as_array().unwrap().len(),2);
    assert!(asked["among"].as_array().unwrap().contains(&authored));
    let answer = json!({"kind":"choose", "answer":{"replacement":{"source":maze.0,"ability":0,"part":0}}});
    g.act(0, read_move(&answer, &Pool::default()).unwrap()).unwrap();
    assert!(g.decision.is_none());
    let arrived = *g.battlefield.last().unwrap();
    assert!(g.obj(arrived).tapped);
    assert_eq!(g.obj(arrived).card, Some(1));
    assert!(g.players[0].land_played);
}

#[cfg(test)]
mod correlation_tests {
    use super::*;

    #[test]
    fn harness_capture_random_cannot_be_requested_as_a_seat_projection() {
        // Uncited (protocol): a failed privileged request must not execute a move.
        let defs=vec![game::CardDef::new("Mix","{0}","Instant","Shuffle. Draw a card.",None).unwrap(),
            game::CardDef::new("Forest","","Basic Land — Forest","",None).unwrap()];
        let mut g=Game::new(defs,2,20);g.put(0,1,Zone::Library);g.put(0,1,Zone::Library);
        let spell=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![]}).unwrap();g.act(g.priority,Move::Pass).unwrap();
        let mut simulation=Some(Simulation::new(g));let mut session=7;let mut logged=Delivery::default();
        let catalog=Catalog::new(BTreeMap::new());
        let mut send=|input:Value|dispatch(&catalog,&mut simulation,&mut session,&mut logged,&input);
        let before=send(json!({"cmd":"state"})).unwrap();
        let error=send(json!({"cmd":"act","session_id":7,"request_id":before["request_id"],"player":before["asked"]["player"],
            "move":{"kind":"pass"},"capture_random":true,"viewer":0})).unwrap_err();
        assert!(error.contains("privileged"));
        let after=send(json!({"cmd":"state"})).unwrap();
        assert_eq!(after["request_id"],before["request_id"]);assert_eq!(after["stack"],before["stack"]);
        assert_eq!(after["players"][0]["library"],before["players"][0]["library"]);
        assert!(after.get("random_outcomes").is_none());
    }

    #[test]
    fn harness_audit_objects_is_opt_in_and_session_bound() {
        // Uncited (protocol): this observes the effective state; it adds no game semantics.
        let defs = vec![game::CardDef::new("Runner", "{R}", "Creature", "Haste", Some((1, 1))).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let runner = g.put(0, 0, Zone::Battlefield);
        let mut simulation = Some(Simulation::new(g));
        let mut session = 7;
        let mut logged = Delivery::default();
        let catalog = Catalog::new(BTreeMap::new());
        let mut send = |input: Value| dispatch(&catalog, &mut simulation, &mut session, &mut logged, &input);
        let regular = send(json!({"cmd":"state"})).unwrap();
        assert!(regular.get("audit").is_none());
        assert!(regular["objects"][runner.0.to_string()].get("keywords").is_none());
        let audit = send(json!({"cmd":"audit_objects","session_id":7})).unwrap();
        assert_eq!(audit["objects"][runner.0.to_string()]["keywords"], json!(["haste"]));
        assert!(send(json!({"cmd":"audit_objects","session_id":6})).unwrap_err().contains("stale session"));
        assert!(send(json!({"cmd":"audit_objects","session_id":7,"viewer":0})).unwrap_err().contains("privileged"));
    }

    #[test]
    fn harness_orders_libraries_and_supplies_shuffle_before_draw() {
        let defs = vec![
            game::CardDef::new("Mix", "{0}", "Instant", "Shuffle. Draw a card.", None).unwrap(),
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            game::CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let forest = g.put(0, 1, Zone::Library);
        let island = g.put(0, 2, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(g.priority, Move::Pass).unwrap();
        let mut simulation = Some(Simulation::new(g));
        let mut session = 7;
        let mut logged = Delivery::default();
        let catalog = Catalog::new(BTreeMap::new());
        let mut send = |input: Value| dispatch(&catalog, &mut simulation, &mut session, &mut logged, &input);
        let first = send(json!({"cmd":"state"})).unwrap();
        let get = json!({"cmd":"library_order","session_id":7,"player":0});
        assert_eq!(send(get.clone()).unwrap()["order"], json!([island.0, forest.0]));
        let mut set = json!({"cmd":"set_library_order","session_id":7,"request_id":first["request_id"],
                            "player":0,"order":[forest.0, forest.0]});
        assert!(send(set.clone()).unwrap_err().contains("permutation"));
        set["order"] = json!([forest.0, island.0]);
        let next = send(set.clone()).unwrap();
        assert_ne!(next["request_id"], first["request_id"]);
        assert!(send(set).unwrap_err().contains("stale request"));
        assert_eq!(send(get.clone()).unwrap()["order"], json!([forest.0, island.0]));
        let mut stale = get.clone(); stale["session_id"] = json!(6);
        assert!(send(stale).unwrap_err().contains("stale session"));
        let done = send(json!({"cmd":"act","session_id":7,"request_id":next["request_id"],
            "player":next["asked"]["player"],"move":{"kind":"pass"},
            "shuffles":[{"player":0,"order":[island.0,forest.0]}]})).unwrap();
        assert_eq!(send(get).unwrap()["order"], json!([forest.0]));
        let drawn = done["players"][0]["hand"][0].as_u64().unwrap().to_string();
        assert_eq!(done["objects"][drawn]["name"], "Island");
        let refused=send(json!({"cmd":"answer","shuffles":[]})).unwrap_err();
        assert_eq!(refused.class(), Some(Class::Malformed));
        assert!(refused.contains("answer has no \"shuffles\""));
    }

    /// `audit_game` as the pipe answers it, of this game.
    fn audited(g: &Game) -> Value {
        let mut simulation = Some(Simulation::new(g.clone()));
        let (mut session, mut logged) = (3, Delivery::default());
        let catalog = Catalog::new(BTreeMap::new());
        dispatch(&catalog, &mut simulation, &mut session, &mut logged, &json!({"cmd":"audit_game","session_id":3})).unwrap()
    }

    #[test]
    fn r510_4_audit_game_counts_combat_damage_steps_and_names_the_first_strikers() {
        // 510.4: with a first striker attacking, the first combat damage step is its alone
        // and a second combat damage step follows for the creature without first strike.
        let defs = vec![
            CardDef::new("Striker", "", "Creature — Soldier", "First strike", Some((2, 2))).unwrap(),
            CardDef::new("Bear", "", "Creature — Bear", "", Some((2, 2))).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let striker = g.put(0, 0, Zone::Battlefield);
        let bear = g.put(0, 1, Zone::Battlefield);
        assert_eq!(audited(&g)["combat"]["damage_steps"], 0);
        assert_eq!(audited(&g)["combat"]["first_strikers"], Value::Null);
        while g.part != ir::Part::DeclareAttackers { g.act(g.priority, Move::Pass).unwrap(); }
        g.act(0, Move::Attack(vec![(striker, Target::Player(1)), (bear, Target::Player(1))])).unwrap();
        while g.part != ir::Part::CombatDamage {
            match g.decision { Some(game::state::Decision::Blockers { player }) => g.act(player, Move::Block(vec![])).unwrap(),
                _ => g.act(g.priority, Move::Pass).unwrap() }
        }
        let first = audited(&g);
        assert_eq!(first["combat"]["damage_steps"], 1);
        assert_eq!(first["combat"]["first_strikers"], json!([striker.0]));
        assert_eq!(g.players[1].life, 18, "only the first striker dealt damage in the first step");
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        assert_eq!(g.part, ir::Part::CombatDamage);
        let second = audited(&g);
        assert_eq!(second["combat"]["damage_steps"], 2);
        assert_eq!(second["combat"]["first_strikers"], json!([striker.0]));
        assert_eq!(g.players[1].life, 16);
    }

    #[test]
    fn r707_2_audit_game_shows_base_values_apart_from_keywords_held() {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let splicer = CardDef::from_face(faces.iter().find(|f| f.face_name == "Blade Splicer").unwrap()).unwrap();
        let wings = CardDef::new("Wings", "", "Creature — Bird", "Flying, haste", Some((1, 1))).unwrap();
        let mut g = Game::new(vec![splicer, wings], 2, 20);
        let bird = g.put(0, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        g.players[0].pool.colors[0] = 1; g.players[0].pool.colorless = 2;
        g.act(0, paid("CCW", Move::Cast { card, targets: vec![] })).unwrap();
        while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        let token = *g.battlefield.iter().find(|id| g.obj(**id).token()).expect("Blade Splicer made its Golem (111.1)");
        let audit = audited(&g);
        let golem = &audit["objects"][token.0.to_string()];
        // The token's copiable values are those its creating effect said (111.3, 707.2):
        // a 3/3 colorless Phyrexian Golem artifact creature with no abilities. First strike
        // is Blade Splicer's grant (613.1f), held but not part of the base.
        assert_eq!(golem["base"], json!({"name":"Phyrexian Golem Token","colors":[],"supertypes":[],
            "types":["Artifact","Creature"],"subtypes":["Phyrexian","Golem"],"power":3,"toughness":3,"keywords":[],"rows":[]}));
        assert_eq!(golem["held"], json!(["first strike"]));
        assert_eq!(golem["keywords"], json!(["first strike"]));
        assert_eq!(golem["properties"]["supertypes"], json!([]));
        assert_eq!(golem["abilities_removed"], false);
        let bird = &audit["objects"][bird.0.to_string()];
        assert_eq!(bird["base"]["keywords"], json!(["flying", "haste"]));
        assert_eq!(bird["base"]["rows"], json!(["Flying, haste"]));
        assert_eq!(bird["held"], json!(["flying", "haste"]));
    }

    #[test]
    fn harness_captures_random_bottom_and_recovers_from_missing_receipt() {
        let defs = vec![
            game::CardDef::new("Random placement", "{0}", "Instant", "Look at the top four cards of your library. Put one of them into your hand and the rest on the bottom of your library in a random order.", None).unwrap(),
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            game::CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let cards: Vec<_> = [1, 1, 2].iter().map(|def| g.put(0, *def, Zone::Library)).collect();
        let physical: Vec<_> = cards.iter().map(|id| g.physical_card_identity(*id).unwrap()).collect();
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(g.priority, Move::Pass).unwrap();
        g.act(g.priority, Move::Pass).unwrap();
        let mut replay = Some(Simulation::new(g));
        let mut recording = replay.clone();
        let catalog = Catalog::new(BTreeMap::new());
        let (mut session, mut logged) = (5, Delivery::default());
        let initial = dispatch(&catalog, &mut replay, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let action = json!({"cmd":"act","session_id":5,"request_id":initial["request_id"],"player":0,
            "move":{"kind":"choose","answer":{"objects":[cards[0].0]}}});
        let mut bad = action.clone(); bad["random_outcomes"] = json!([]);
        let failure = dispatch(&catalog, &mut replay, &mut session, &mut logged, &bad).unwrap_err();
        assert!(failure.starts_with("replay input: "), "{failure}");
        assert!(replay.as_ref().unwrap().fault().is_none());
        let pending = dispatch(&catalog, &mut replay, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        assert_eq!(pending["request_id"], initial["request_id"]);
        let mut capture = action.clone(); capture["capture_random"] = json!(true);
        let captured = dispatch(&catalog, &mut recording, &mut session, &mut Delivery::default(), &capture).unwrap();
        let receipt = &captured["random_outcomes"][0];
        assert_eq!(receipt["kind"], "random_order");
        assert_eq!(receipt["position"], "bottom");
        assert_eq!(receipt["player"], 0);
        let order: Vec<u64> = receipt["physical_order"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap()).collect();
        let mut members = order.clone(); members.sort();
        let mut expected = vec![physical[1], physical[2]]; expected.sort();
        assert_eq!(members, expected);
        let objects: Vec<_> = order.iter().map(|id| cards[physical.iter().position(|p| p == id).unwrap()].0).collect();
        let mut supplied = action;
        supplied["random_outcomes"] = json!([{"kind":"random_order","order":objects}]);
        dispatch(&catalog, &mut replay, &mut session, &mut logged, &supplied).unwrap();
        let game = replay.as_ref().unwrap().game().unwrap();
        assert_eq!(game.top_cards(0, 3).iter().map(|id| game.physical_card_identity(*id).unwrap()).collect::<Vec<_>>(), order);
        assert_eq!(game.physical_card_identity(game.players[0].hand[0]), Some(physical[0]));
        assert!(!game.log.iter().any(|event| matches!(event, Event::Shuffled { .. })));
    }

    #[test]
    fn harness_keeps_unused_shuffle_outcomes_only_when_asked() {
        let defs = vec![
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
            game::CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        let forest = g.put(0, 0, Zone::Library);
        let island = g.put(0, 1, Zone::Library);
        g.act(g.priority, Move::Pass).unwrap();
        let run = |keep: Option<bool>| {
            let mut simulation = Some(Simulation::new(g.clone()));
            let (mut session, mut logged) = (5, Delivery::default());
            let catalog = Catalog::new(BTreeMap::new());
            let mut send = |input: Value| dispatch(&catalog, &mut simulation, &mut session, &mut logged, &input);
            let state = send(json!({"cmd":"state"})).unwrap();
            let mut act = json!({"cmd":"act","session_id":5,"request_id":state["request_id"],
                "player":state["asked"]["player"],"move":{"kind":"pass"},
                "shuffles":[{"player":0,"order":[island.0,forest.0]}]});
            if let Some(keep) = keep { act["shuffles_keep_unused"] = json!(keep); }
            send(act)
        };
        // Passing priority shuffles nothing, so the supplied outcome is returned unused.
        let kept = run(Some(true)).unwrap();
        assert_eq!(kept["shuffles_unused"], json!([{"player":0,"order":[island.0,forest.0]}]));
        assert!(run(None).unwrap_err().contains("unused shuffle outcomes"));
        assert!(run(Some(false)).unwrap_err().contains("unused shuffle outcomes"));
    }

    #[test]
    fn harness_keeping_unused_shuffles_still_fails_a_shuffle_with_no_outcome() {
        let defs = vec![
            game::CardDef::new("Mix", "{0}", "Instant", "Shuffle. Draw a card.", None).unwrap(),
            game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 1, Zone::Library);
        g.put(0, 1, Zone::Library);
        let spell = g.put(0, 0, Zone::Hand);
        g.act(0, Move::Cast { card: spell, targets: vec![] }).unwrap();
        g.act(g.priority, Move::Pass).unwrap();
        let mut simulation = Some(Simulation::new(g));
        let (mut session, mut logged) = (5, Delivery::default());
        let catalog = Catalog::new(BTreeMap::new());
        let mut send = |input: Value| dispatch(&catalog, &mut simulation, &mut session, &mut logged, &input);
        let state = send(json!({"cmd":"state"})).unwrap();
        assert!(send(json!({"cmd":"act","session_id":5,"request_id":state["request_id"],"player":state["asked"]["player"],
            "move":{"kind":"pass"},"shuffles_keep_unused":true})).unwrap_err().contains("needs shuffles"));
        let missing = send(json!({"cmd":"act","session_id":5,"request_id":state["request_id"],"player":state["asked"]["player"],
            "move":{"kind":"pass"},"shuffles":[],"shuffles_keep_unused":true})).unwrap_err();
        assert!(missing.contains("unexpected extra shuffle"), "{missing}");
    }

    #[test]
    fn protocol_requires_ids_and_rejects_replays_across_resets() {
        let faces = Catalog::new(BTreeMap::new());
        let mut simulation = None;
        let mut session = 0;
        let mut logged = Delivery::default();
        let setup = json!({"cmd":"new", "players":[{}, {}]});
        let first = dispatch(&faces, &mut simulation, &mut session, &mut logged, &setup).unwrap();
        let pass = json!({"cmd":"act", "session_id":first["session_id"], "request_id":first["request_id"],
            "player": first["asked"]["player"], "move":{"kind":"pass"}});
        for field in ["session_id", "request_id"] {
            let mut missing = pass.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(dispatch(&faces, &mut simulation, &mut session, &mut logged, &missing).unwrap_err().contains(field));
        }
        let next = dispatch(&faces, &mut simulation, &mut session, &mut logged, &pass).unwrap();
        assert_ne!(next["request_id"], first["request_id"]);
        assert!(dispatch(&faces, &mut simulation, &mut session, &mut logged, &pass).unwrap_err().contains("stale request"));
        let reset = dispatch(&faces, &mut simulation, &mut session, &mut logged, &setup).unwrap();
        assert_eq!(reset["request_id"], first["request_id"]);
        assert_ne!(reset["session_id"], first["session_id"]);
        assert!(dispatch(&faces, &mut simulation, &mut session, &mut logged, &pass).unwrap_err().contains("stale session"));
    }

    /// Two Forests in player 0's hand, in their first main phase with priority.
    fn two_forests() -> (Simulation, ObjId, ObjId) {
        let mut g = Game::new(vec![game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()], 2, 20);
        let (first, second) = (g.put(0, 0, Zone::Hand), g.put(0, 0, Zone::Hand));
        (Simulation::new(g), first, second)
    }

    /// What a refusal says on the wire: its words, its class and, for an illegal one, its rules.
    fn said(refused: Result<Value, Error>) -> Value { refused.unwrap_err().said() }

    #[test]
    fn every_refusal_says_its_class_and_an_illegal_one_its_rules() {
        let (simulation, first, second) = two_forests();
        let (mut game, mut session, mut logged) = (Some(simulation), 7, Delivery::default());
        let catalog = Catalog::new(BTreeMap::new());
        let state = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let act = |state: &Value, mv: Value| json!({"cmd":"act","session_id":7,"request_id":state["request_id"],
            "player":state["asked"]["player"],"move":mv});
        let mut send = |input: Value| dispatch(&catalog, &mut game, &mut session, &mut logged, &input);
        let played = send(act(&state, json!({"kind":"play","card":first.0}))).unwrap();
        // illegal: one land a turn (305.2)
        assert_eq!(said(send(act(&played, json!({"kind":"play","card":second.0})))),
            json!({"ok":false,"error":"you have played a land this turn (305.2)","class":"illegal","rules":["305.2"]}));
        // unreferable: an object the game does not have, a player it does not have, no game
        assert_eq!(said(send(act(&played, json!({"kind":"play","card":999_999})))),
            json!({"ok":false,"error":"an object handle is stale or unknown","class":"unreferable"}));
        assert_eq!(said(send(json!({"cmd":"library_order","session_id":7,"player":5})))["class"], "unreferable");
        // stale: a request answered already, an outdated session
        assert_eq!(said(send(act(&state, json!({"kind":"pass"})))),
            json!({"ok":false,"error":format!("stale request {}; expected {}", state["request_id"], played["request_id"]),"class":"stale"}));
        let mut outdated = act(&played, json!({"kind":"pass"})); outdated["session_id"] = json!(6);
        assert_eq!(said(send(outdated))["class"], "stale");
        // malformed: a field missing or misspelled, a command the pipe does not have
        assert_eq!(said(send(json!({"cmd":"act","session_id":7,"request_id":played["request_id"],"player":0}))),
            json!({"ok":false,"error":"the request has no move","class":"malformed"}));
        assert_eq!(said(send(act(&played, json!({"kind":"pass","cards":[]}))))["class"], "malformed");
        assert_eq!(said(send(json!({"cmd":"dance"})))["class"], "malformed");
        // the game goes on from where it was refused
        assert_eq!(send(act(&played, json!({"kind":"pass"}))).unwrap()["asked"]["player"], 1);
        let mut none = None;
        assert_eq!(said(dispatch(&catalog, &mut none, &mut 0, &mut Delivery::default(), &json!({"cmd":"state"}))),
            json!({"ok":false,"error":"no game","class":"unreferable"}));

        // unsupported: a concession whose consequence for the pending decision the engine
        // does not run (800.4a), refused with the game as it was
        let mut game = Some(Simulation::new(Game::new(vec![], 3, 20)));
        let (mut session, mut logged) = (1, Delivery::default());
        let state = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let before = format!("{:?}", game.as_ref().unwrap().game().unwrap());
        let concede = json!({"cmd":"act","session_id":1,"request_id":state["request_id"],"player":state["asked"]["player"],
            "move":{"kind":"concede"}});
        let refused = said(dispatch(&catalog, &mut game, &mut session, &mut logged, &concede));
        assert_eq!(refused["class"], "unsupported", "{refused}");
        assert!(refused.get("rules").is_none());
        assert_eq!(format!("{:?}", game.as_ref().unwrap().game().unwrap()), before);
        assert_eq!(game.as_mut().unwrap().request().unwrap().unwrap().id.0, state["request_id"].as_u64().unwrap());
    }

    #[test]
    fn a_malformed_request_changes_nothing_and_consumes_no_request() {
        let (simulation, first, _) = two_forests();
        let (mut game, mut session, mut logged) = (Some(simulation), 7, Delivery::default());
        let catalog = Catalog::new(BTreeMap::new());
        let state = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let play = json!({"cmd":"act","session_id":7,"request_id":state["request_id"],"player":0,
            "move":{"kind":"play","card":first.0}});
        let before = (format!("{:?}", game.as_ref().unwrap()), logged.clone(), session);
        let mut malformed = Vec::new();
        for (field, value) in [("viewer", json!("everyone")), ("move", json!({"kind":"play","card":"Forest"})),
                               ("capture_random", json!(true)), ("shuffles_keep_unused", json!(true)),
                               ("random_outcomes", json!("none")), ("shuffles", json!([{"player":0}]))] {
            let mut bad = play.clone(); bad[field] = value;
            if field == "capture_random" { bad["viewer"] = json!(0); }
            malformed.push(bad);
        }
        for bad in malformed {
            let refused = dispatch(&catalog, &mut game, &mut session, &mut logged, &bad).unwrap_err();
            assert_eq!(refused.class(), Some(Class::Malformed), "{bad}: {refused}");
            assert_eq!((format!("{:?}", game.as_ref().unwrap()), logged.clone(), session), before, "{bad}");
        }
        let played = dispatch(&catalog, &mut game, &mut session, &mut logged, &play).unwrap();
        assert_eq!(played["players"][0]["hand"].as_array().unwrap().len(), 1);
    }

    /// A request naming no viewer it can be shown to is refused before it is carried
    /// out, and the same request can then be made with a viewer.
    #[test]
    fn a_viewer_is_read_before_the_game_is_given_the_request() {
        let (simulation, first, _) = two_forests();
        let (mut game, mut session, mut logged) = (Some(simulation), 7, Delivery::default());
        let catalog = Catalog::new(BTreeMap::new());
        let state = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let play = json!({"cmd":"act","session_id":7,"request_id":state["request_id"],"player":0,
            "move":{"kind":"play","card":first.0}});
        let before = (format!("{:?}", game.as_ref().unwrap()), logged.clone());
        for (viewer, class) in [(json!(2), Class::Unreferable), (json!(999), Class::Unreferable), (json!(-1), Class::Malformed),
                                (json!("p0"), Class::Malformed)] {
            let mut invalid = play.clone(); invalid["viewer"] = viewer.clone();
            let refused = dispatch(&catalog, &mut game, &mut session, &mut logged, &invalid).unwrap_err();
            assert_eq!(refused, format!("a viewer is a player or \"asked\", not {viewer}"));
            assert_eq!(refused.class(), Some(class), "{viewer}");
            assert_eq!((format!("{:?}", game.as_ref().unwrap()), logged.clone()), before, "{viewer}");
        }
        let mut shown = play.clone(); shown["viewer"] = json!(0);
        let played = dispatch(&catalog, &mut game, &mut session, &mut logged, &shown).unwrap();
        assert_eq!(played["players"][0]["hand"].as_array().unwrap().len(), 1);
        assert_ne!(played["request_id"], state["request_id"]);

        // and so for setting a library's order
        let defs = vec![game::CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let (bottom, top) = (g.put(0, 0, Zone::Library), g.put(0, 0, Zone::Library));
        let (mut game, mut logged) = (Some(Simulation::new(g)), Delivery::default());
        let state = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"state"})).unwrap();
        let set = json!({"cmd":"set_library_order","session_id":7,"request_id":state["request_id"],"player":0,
            "order":[bottom.0, top.0]});
        let before = (format!("{:?}", game.as_ref().unwrap()), logged.clone());
        for (field, value, class) in [("viewer", json!(999), Class::Unreferable), ("viewer", json!("all"), Class::Malformed),
                                      ("player", json!(2), Class::Unreferable)] {
            let mut invalid = set.clone(); invalid[field] = value.clone();
            let refused = dispatch(&catalog, &mut game, &mut session, &mut logged, &invalid).unwrap_err();
            assert_eq!(refused.class(), Some(class), "{field} {value}: {refused}");
            assert_eq!((format!("{:?}", game.as_ref().unwrap()), logged.clone()), before, "{field} {value}");
        }
        let mut shown = set; shown["viewer"] = json!(1);
        let next = dispatch(&catalog, &mut game, &mut session, &mut logged, &shown).unwrap();
        assert_ne!(next["request_id"], state["request_id"]);
        let order = dispatch(&catalog, &mut game, &mut session, &mut logged, &json!({"cmd":"library_order","session_id":7,"player":0})).unwrap();
        assert_eq!(order["order"], json!([bottom.0, top.0]));
    }

    #[test]
    fn failed_setup_does_not_publish_a_session_or_replace_the_previous_game() {
        let mut game = Some(Simulation::new(Game::new(vec![], 2, 20)));
        let request = game.as_mut().unwrap().request().unwrap().unwrap().clone();
        let mut session = 7;
        let mut logged = Delivery { rendered: game::event::Seq(3), ..Delivery::default() };
        let before = format!("{:?}", game.as_ref().unwrap());
        let failure = reset(&mut game, &mut session, &mut logged, &json!({}), || {
            let mut partial = Game::new(vec![], 2, 20);
            partial.players[0].life = 1;
            panic!("controlled setup failure with private diagnostics");
        });
        assert_eq!(failure.unwrap_err(), "simulation setup fault");
        assert_eq!(session, 7);
        assert_eq!(logged, Delivery { rendered: game::event::Seq(3), ..Delivery::default() });
        assert_eq!(format!("{:?}", game.as_ref().unwrap()), before);
        assert_eq!(game.as_mut().unwrap().request().unwrap(), Some(&request));
        // A setup that returns but fails while forming its first request is unpublished too.
        assert_eq!(reset(&mut game, &mut session, &mut logged, &json!({}), || {
            let mut malformed = Game::new(vec![], 2, 20);
            malformed.decision = Some(Decision::Discard { player: 2, count: 1 });
            Ok(malformed)
        }).unwrap_err(), "simulation setup fault");
        assert_eq!(session, 7);
        assert_eq!(format!("{:?}", game.as_ref().unwrap()), before);
        // Setup succeeds, but its first viewer projection rejects the request.
        for invalid in [json!({"cmd":"new", "players":[{}, {}, {}], "viewer":999}),
                        json!({"cmd":"start", "decks":[[], [], []], "viewer":999})] {
            assert_eq!(dispatch(&Catalog::new(BTreeMap::new()), &mut game, &mut session, &mut logged, &invalid).unwrap_err(),
                "a viewer is a player or \"asked\", not 999");
            assert_eq!(session, 7);
            assert_eq!(logged, Delivery { rendered: game::event::Seq(3), ..Delivery::default() });
            assert_eq!(format!("{:?}", game.as_ref().unwrap()), before);
            assert_eq!(game.as_mut().unwrap().request().unwrap(), Some(&request));
        }
        // Deliberately malformed fixture: the request is valid, but snapshotting faults.
        // The candidate's diagnostics must not escape when that candidate is discarded.
        let malformed = || {
            let mut game = Game::new(vec![], 2, 20);
            game.players[0].hand.push(ObjId(123456));
            Ok(game)
        };
        assert_eq!(reset(&mut game, &mut session, &mut logged, &json!({}), malformed).unwrap_err(),
            "simulation setup fault");
        assert_eq!(session, 7);
        assert_eq!(logged, Delivery { rendered: game::event::Seq(3), ..Delivery::default() });
        assert_eq!(format!("{:?}", game.as_ref().unwrap()), before);
        game.as_mut().unwrap().respond(request.id, request.player, Move::Pass).unwrap();

        let invalid = json!({"cmd":"start", "decks":[]});
        let error = dispatch(&Catalog::new(BTreeMap::new()), &mut game, &mut session, &mut logged, &invalid).unwrap_err();
        assert_eq!(error, "a game has at least two players");
        assert_eq!(session, 7);
        assert!(game.as_ref().unwrap().fault().is_none());

        let mut empty = None;
        let mut initial_session = 0;
        assert_eq!(reset(&mut empty, &mut initial_session, &mut Delivery::default(), &json!({}), || {
            panic!("controlled first setup failure");
        }), Err(Error::Fault("simulation setup fault".into())));
        assert!(empty.is_none());
        assert_eq!(initial_session, 0);
        assert_eq!(reset(&mut empty, &mut initial_session, &mut Delivery::default(), &json!({}), malformed).unwrap_err(),
            "simulation setup fault");
        assert!(empty.is_none());
        assert_eq!(initial_session, 0);
    }

    #[test]
    fn the_engine_says_which_it_is_with_or_without_a_game_and_changes_nothing() {
        let (mut session, faces, mut logged) = (3, Catalog::new(BTreeMap::new()), Delivery::default());
        let mut none = None;
        let said = dispatch(&faces, &mut none, &mut session, &mut logged, &json!({"cmd":"version"})).unwrap();
        assert_eq!(said, json!({"version": version()}));
        let v = &said["version"];
        assert_eq!((&v["apis"]["harness"], &v["apis"]["player"], &v["random"]), (&json!("harness/1"), &json!(game::prompt::API), &json!(game::start::RANDOM)));
        assert_eq!(v["package"], env!("CARGO_PKG_VERSION"));
        // what the builder said, or nothing: never a guess
        match option_env!("GAME_SOURCE_REVISION") {
            Some(revision) if !revision.is_empty() => assert_eq!(v["source"]["revision"], revision),
            _ => assert!(v["source"].is_null()),
        }
        assert!(none.is_none());
        assert_eq!(session, 3);
    }

    #[test]
    fn a_harness_reads_which_object_each_label_names_and_changes_nothing() {
        let zap = CardDef::new("Zap", "{0}", "Instant", "Zap deals 1 damage to any target.", None).unwrap();
        let secret = CardDef::new("Secret", "{0}", "Instant", "You gain 1 life.", None).unwrap();
        let mut game = Game::new(vec![zap, secret], 2, 20);
        let (zap, secret) = (game.put(0, 0, Zone::Hand), game.put(1, 1, Zone::Hand));
        let mut simulation = Some(Simulation::new(game));
        let (mut session, faces, mut logged) = (1, Catalog::new(BTreeMap::new()), Delivery::default());
        let mut send = |simulation: &mut Option<Simulation>, request: Value| dispatch(&faces, simulation, &mut session, &mut logged, &request);
        // nothing given yet: no label names anything
        assert_eq!(send(&mut simulation, json!({"cmd":"labels","seat":"p0"})).unwrap(), json!({"labels":{}}));
        let prompt = send(&mut simulation, json!({"cmd":"prompt","seat":"p0"})).unwrap();
        let label = prompt["players"][0]["hand"][0].as_str().unwrap().to_owned();
        let before = send(&mut simulation, json!({"cmd":"state"})).unwrap();
        let said = send(&mut simulation, json!({"cmd":"labels","seat":"p0"})).unwrap();
        assert_eq!(said, json!({"labels":{label.clone(): zap.0}}));
        // the other seat's card was never shown to p0, and p1 was shown nothing
        assert_ne!(said["labels"][&label], json!(secret.0));
        assert_eq!(send(&mut simulation, json!({"cmd":"labels","seat":"p1"})).unwrap(), json!({"labels":{}}));
        assert_eq!(send(&mut simulation, json!({"cmd":"state"})).unwrap()["objects"], before["objects"]);
        // the next prompt names the same object by the same label, and no new one
        assert_eq!(send(&mut simulation, json!({"cmd":"prompt","seat":"p0"})).unwrap()["players"][0]["hand"][0], json!(label));
        assert_eq!(send(&mut simulation, json!({"cmd":"labels","seat":"p9"})), Err(Error::unreferable("the game has no player 9")));
        assert_eq!(send(&mut simulation, json!({"cmd":"labels","seat":"p0"})).unwrap(), said);
        assert_eq!(send(&mut simulation, json!({"cmd":"state"})).unwrap()["objects"], before["objects"]);
    }

    /// A prompt says each characteristic of an object its seat may look at (109.3): the
    /// Adventure an adventurer card has without using it (715.2a), supertypes (205.4a), and
    /// off the battlefield the power, toughness and loyalty printed (208.3, 306.5a).
    #[test]
    fn r109_3_a_prompt_says_an_inset_supertypes_and_printed_numbers() {
        let giant = CardDef::new("Giant", "{6}{G}", "Legendary Creature — Giant", "", Some((7, 7))).unwrap();
        let journey = CardDef::new("Journey", "{2}{G}", "Sorcery — Adventure", "You gain 3 life.", None).unwrap();
        let defs = vec![
            CardDef::adventurer(giant, journey).unwrap(),
            CardDef::planeswalker("Walker", "{0}", "Legendary Planeswalker — Jace", "+1: You gain 1 life.", 4).unwrap(),
            CardDef::new("Cart", "{1}", "Artifact — Vehicle", "Crew 1", Some((3, 3))).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        let (giant, walker, cart) = (game.put(0, 0, Zone::Hand), game.put(0, 1, Zone::Hand), game.put(0, 2, Zone::Battlefield));
        let mut simulation = Some(Simulation::new(game));
        let (mut session, faces, mut logged) = (1, Catalog::new(BTreeMap::new()), Delivery::default());
        let mut send = |request: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &request).unwrap();
        let prompt = send(json!({"cmd":"prompt","seat":"p0"}));
        let labels = send(json!({"cmd":"labels","seat":"p0"}));
        let object = |id: ObjId| {
            let (label, _) = labels["labels"].as_object().unwrap().iter().find(|(_, o)| **o == json!(id.0)).expect("labelled");
            prompt["objects"][label].as_object().unwrap().clone()
        };
        let giant = object(giant);
        assert_eq!(giant["adventure"], json!({"name":"Journey", "mana_cost":"{2}{G}", "supertypes":[], "types":["Sorcery"],
            "subtypes":["Adventure"], "colors":["green"], "text":["You gain 3 life."]}));
        assert_eq!((&giant["mana_cost"], &giant["supertypes"]), (&json!("{6}{G}"), &json!(["Legendary"])));
        assert_eq!((&giant["power"], &giant["toughness"]), (&json!(7), &json!(7)));
        let walker = object(walker);
        assert_eq!((&walker["loyalty"], &walker["supertypes"]), (&json!(4), &json!(["Legendary"])));
        assert!(!walker.contains_key("power"));
        // a noncreature permanent has no power or toughness (208.3, 301.7a), nor printed loyalty
        let cart = object(cart);
        assert!(!cart.contains_key("power") && !cart.contains_key("toughness") && !cart.contains_key("loyalty"), "{cart:?}");
    }

    /// An ability on the stack has the text of the ability that created it and no other
    /// characteristics (405.4, 603.3), on the wire too: the ward trigger of a cloaked legendary
    /// adventurer says to neither seat, nor to the harness, its card's cost, Adventure, power,
    /// supertypes or abilities, which its opponent may not look at (708.5).
    #[test]
    fn r405_4_an_ability_on_the_stack_is_said_with_its_text_alone() {
        let giant = CardDef::new("Giant", "{6}{G}", "Legendary Creature — Giant", "{1}: You gain 1 life.", Some((7, 7))).unwrap();
        let journey = CardDef::new("Journey", "{2}{G}", "Sorcery — Adventure", "You gain 3 life.", None).unwrap();
        let defs = vec![
            CardDef::new("Cloaker", "{0}", "Sorcery", "Cloak the top card of your library.", None).unwrap(),
            CardDef::adventurer(giant, journey).unwrap(),
            CardDef::new("Zap", "{0}", "Instant", "Zap deals 2 damage to any target.", None).unwrap(),
            CardDef::new("Wastes", "", "Basic Land", "", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..5 { game.put(p, 3, Zone::Library); } }
        game.put(0, 1, Zone::Library);
        let cloaker = game.put(0, 0, Zone::Hand);
        game.act(0, Move::Cast { card: cloaker, targets: vec![] }).unwrap();
        for p in [0, 1, 0] { game.act(p, Move::Pass).unwrap(); }
        let cloaked = *game.battlefield.iter().find(|o| game.face_down(**o)).expect("the Giant is cloaked");
        let zap = game.put(1, 2, Zone::Hand);
        game.act(1, Move::Cast { card: zap, targets: vec![vec![game::Target::Object(cloaked)]] }).unwrap();
        let ward = *game.stack.last().unwrap();
        assert!(game.obj(ward).ability.is_some(), "the ward trigger is on the stack");
        let mut simulation = Some(Simulation::new(game));
        let (mut session, faces, mut logged) = (1, Catalog::new(BTreeMap::new()), Delivery::default());
        let mut send = |request: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &request).unwrap();
        let none = |o: &serde_json::Map<String, Value>, viewer: &str| {
            for field in ["mana_cost", "adventure", "omen", "prepare_spell", "power", "toughness", "loyalty", "abilities", "card_text", "face_down"] {
                assert!(!o.contains_key(field), "{viewer}: {field} in {o:?}");
            }
            for field in ["types", "subtypes", "supertypes", "colors"] { assert_eq!(o[field], json!([]), "{viewer}: {field}"); }
            // its name is its face-down source's, which has none (708.2a)
            assert_eq!((&o["name"], &o["ability"]), (&json!(""), &json!(true)), "{viewer}");
        };
        for seat in ["p0", "p1"] {
            let prompt = send(json!({"cmd":"prompt","seat":seat}));
            let labels = send(json!({"cmd":"labels","seat":seat}));
            let (label, _) = labels["labels"].as_object().unwrap().iter().find(|(_, o)| **o == json!(ward.0)).expect("labelled");
            let o = prompt["objects"][label].as_object().unwrap();
            none(o, seat);
            // its text is the one line of ward's trigger (702.21a), as its occurrence says it
            assert_eq!(o["text"].as_array().map(Vec::len), Some(1), "{seat}: {o:?}");
            assert_eq!(o["text"][0], o["occurrence"]["text"], "{seat}");
            assert!(o["text"][0].as_str().unwrap().contains("{2}"), "{seat}: {o:?}");
        }
        let state = send(json!({"cmd":"state"}));
        none(state["objects"][key(ward)].as_object().expect("the harness sees it"), "the harness");
    }

    /// Every label (`o12`, `o12.a0`) a value says.






    #[test]
    fn r601_2h_json_payment_can_be_corrected_or_reversed_after_explicit_ordering() {
        for reverse in [false, true] {
            let defs = vec![
                CardDef::new("Source", "{0}", "Creature — Elf", "{T}: Put a +1/+1 counter on this creature. Add X mana of any one color, where X is the number of +1/+1 counters on this creature.", Some((1,1))).unwrap(),
                CardDef::new("Double", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, twice that many +1/+1 counters are put on that creature instead.", None).unwrap(),
                CardDef::new("Extra", "{0}", "Enchantment", "If one or more +1/+1 counters would be put on a creature you control, that many plus one +1/+1 counters are put on it instead.", None).unwrap(),
                CardDef::new("Spell", "{4}", "Sorcery", "You gain 3 life.", None).unwrap(),
                CardDef::new("Land", "", "Land", "{T}: Add {G}.", None).unwrap(),
            ];
            for d in &defs { game::play::check(d).unwrap(); }
            let mut g = Game::new(defs, 2, 20);
            let source = g.put(0, 0, Zone::Battlefield);
            let double = g.put(0, 1, Zone::Battlefield);
            g.put(0, 2, Zone::Battlefield);
            let card = g.put(0, 3, Zone::Hand);
            let land = g.put(0, 4, Zone::Battlefield);
            let mut simulation = Some(Simulation::new(g));
            let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
            let mut say = |v: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &v);
            let first = say(json!({"cmd":"state"})).unwrap();
            let pending = say(json!({"cmd":"act","session_id":1,"request_id":first["request_id"],"player":0,
                "move":{"kind":"cast","card":card.0,"plan":[{"source":source.0,"ability":0,"adding":[["G","G","G"]]}]}})).unwrap();
            assert_eq!(pending["asked"]["kind"], "replacement");
            let pending = say(json!({"cmd":"act","session_id":1,"request_id":pending["request_id"],"player":0,
                "move":{"kind":"choose","answer":{"replacement":{"source":double.0,"ability":0,"part":0}}}})).unwrap();
            assert_eq!(pending["asked"]["kind"], "payment");
            assert_eq!(pending["asked"]["can_reverse"], true);
            let action = if reverse { json!({"kind":"reverse_payment"}) }
                else { json!({"kind":"payment","mana":{"G":4},"plan":[{"source":land.0,"ability":0}]}) };
            let complete = say(json!({"cmd":"act","session_id":1,"request_id":pending["request_id"],"player":0,"move":action})).unwrap();
            assert_eq!(complete["asked"]["kind"], "priority");
            assert_ne!(complete["request_id"], pending["request_id"]);
            let g = simulation.as_ref().unwrap().game().unwrap();
            assert_eq!(g.stack.len(), if reverse { 0 } else { 1 });
            assert_eq!(g.players[0].pool.total(), if reverse { 3 } else { 0 });
            if reverse { assert!(g.players[0].hand.contains(&card)); }
        }
    }

    /// Explore's "may" prompt must come with the reveal in its events.
    #[test]
    fn r701_44a_explores_may_prompt_comes_with_the_revealed_card() {
        let scout = CardDef::new("Scout", "{0}", "Creature — Elf", "When this creature enters, it explores.", Some((1, 1))).unwrap();
        let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
        let mut game = Game::new(vec![scout, bear], 2, 20);
        let card = game.put(0, 0, Zone::Hand);
        let top = game.put(0, 1, Zone::Library);
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |v: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &v);
        let act = |request: &Value, player: u64, mv: Value| json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv, "viewer":"asked"});
        let mut now = say(json!({"cmd":"state"})).unwrap();
        now = say(act(&now["request_id"], 0, json!({"kind":"cast", "card":card.0}))).unwrap();
        let mut events = Vec::new();
        for _ in 0..8 {
            if now["asked"]["kind"] == "may" { break; }
            let player = now["asked"]["player"].as_u64().unwrap();
            now = say(act(&now["request_id"], player, json!({"kind":"pass"}))).unwrap();
            events.extend(now["events"].as_array().cloned().unwrap_or_default());
        }
        assert_eq!(now["asked"]["kind"], "may", "{now}");
        assert_eq!(now["asked"]["optional"], json!({"verb":"Move","to":{"zone":"graveyard"}}), "explore may put the card into the graveyard (701.44a)");
        assert!(events.iter().any(|e| e["event"] == "revealed" && e["cards"] == json!([top.0])), "{events:?}");
        assert!(now["events"].as_array().unwrap().iter().any(|e| e["event"] == "revealed"), "in the response that asks: {}", now["events"]);
    }

    /// The same of an activated ability's explore (a Map token's, 111.10s).
    #[test]
    fn r701_44a_an_activated_explores_may_prompt_comes_with_the_revealed_card() {
        let map = CardDef::new("Map", "{0}", "Artifact", "{T}, Sacrifice this artifact: Target creature you control explores. Activate only as a sorcery.", None).unwrap();
        let bear = CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap();
        let mut game = Game::new(vec![map, bear], 2, 20);
        let source = game.put(0, 0, Zone::Battlefield);
        let explorer = game.put(0, 1, Zone::Battlefield);
        let top = game.put(0, 1, Zone::Library);
        while game.part != ir::Part::PrecombatMain { game.act(game.priority, Move::Pass).unwrap(); }
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |v: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &v);
        let act = |request: &Value, player: u64, mv: Value| json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv, "viewer":"asked"});
        let mut now = say(json!({"cmd":"state"})).unwrap();
        now = say(act(&now["request_id"], 0, json!({"kind":"activate", "source":source.0, "ability":0, "targets":[[{"object":explorer.0}]]}))).unwrap();
        for _ in 0..8 {
            if now["asked"]["kind"] == "may" { break; }
            let player = now["asked"]["player"].as_u64().unwrap();
            now = say(act(&now["request_id"], player, json!({"kind":"pass"}))).unwrap();
        }
        assert_eq!(now["asked"]["kind"], "may", "{now}");
        assert_eq!(now["asked"]["optional"], json!({"verb":"Move","to":{"zone":"graveyard"}}), "explore may put the card into the graveyard (701.44a)");
        assert!(now["events"].as_array().unwrap().iter().any(|e| e["event"] == "revealed" && e["cards"] == json!([top.0])), "{}", now["events"]);
    }



    #[test]
    fn what_a_seat_was_shown_is_delivered_once_to_it_with_no_question_following() {
        let def = |name: &str, text: &str| CardDef::new(name, "{0}", "Instant", text, None).unwrap();
        let defs = vec![def("Glance", "Look at the top two cards of your library."), def("Show", "Reveal the top card of your library."),
            CardDef::new("Wolf", "{0}", "Creature — Wolf", "", Some((3, 3))).unwrap(), CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((2, 2))).unwrap()];
        let mut game = Game::new(defs, 2, 20);
        let bear = game.put(0, 3, Zone::Library);
        let wolf = game.put(0, 2, Zone::Library);
        let (glance, show) = (game.put(0, 0, Zone::Hand), game.put(0, 1, Zone::Hand));
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        let resolve = |say: &mut dyn FnMut(Value) -> Value, card: ObjId| {
            for (player, mv) in [(0, json!({"kind":"cast", "card":card.0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
                let request = say(json!({"cmd":"state"}))["request_id"].clone();
                say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv}));
            }
        };
        resolve(&mut say, glance);
        // the whole-game view of a harness is no seat: it has no observations and takes none
        assert!(say(json!({"cmd":"state"})).get("observations").is_none());
        // the look is over, priority is asked, and the seat that looked is told what it saw
        let mine = say(json!({"cmd":"state", "viewer":0}));
        assert_eq!(mine["asked"]["kind"], "priority");
        assert_eq!(mine["observations"], json!([{"seq":0, "kind":"looked", "by":0, "cards":[
            {"object":wolf.0, "name":"Wolf", "owner":0, "zone":"library"}, {"object":bear.0, "name":"Bear", "owner":0, "zone":"library"}]}]));
        // the library is hidden again: the cards are not among the objects shown now
        assert!(mine["objects"].get(wolf.0.to_string()).is_none());
        // asked again, nothing is repeated; the other seat was shown nothing
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["observations"], json!([]));
        assert_eq!(say(json!({"cmd":"state", "viewer":1}))["observations"], json!([]));
        // a reveal goes to both seats, each once, whichever looks first
        resolve(&mut say, show);
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!(theirs["observations"], json!([{"seq":1, "kind":"revealed", "by":0, "cards":[{"object":wolf.0, "name":"Wolf", "owner":0, "zone":"library"}]}]));
        assert_eq!(say(json!({"cmd":"state", "viewer":1}))["observations"], json!([]));
        assert_eq!(say(json!({"cmd":"state", "viewer":"asked"}))["observations"], theirs["observations"]);
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["observations"], json!([]));
    }

    /// Cards exiled face down through the pipe (406.3, 406.3a): every seat is shown a card
    /// with no name or types and who may look at it; the player who looked is shown which
    /// card, without asking to look; and nothing names it to the other seat.
    #[test]
    fn r406_3_face_down_exile_through_the_pipe() {
        let defs = vec![
            CardDef::new("Seer", "{0}", "Sorcery", "Look at the top two cards of your library. Exile one of them face down and put the rest on the bottom of your library in a random order.", None).unwrap(),
            CardDef::new("Vault", "{0}", "Sorcery", "Exile the top card of your library face down.", None).unwrap(),
            CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Elf", "{G}", "Creature — Elf", "", Some((1, 1))).unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { game.put(p, 4, Zone::Library); } }
        game.put(0, 3, Zone::Library);
        game.put(0, 2, Zone::Library);
        let seer = game.put(0, 0, Zone::Hand);
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        let act = |say: &mut dyn FnMut(Value) -> Value, player: usize, mv: Value| {
            let request = say(json!({"cmd":"state"}))["request_id"].clone();
            say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv}))
        };
        // drain what each seat was told before, to read only what this exile tells them
        say(json!({"cmd":"state", "viewer":1}));
        for (player, mv) in [(0, json!({"kind":"cast", "card":seer.0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
            act(&mut say, player, mv);
        }
        let asked = say(json!({"cmd":"state", "viewer":0}))["asked"].clone();
        let mine = say(json!({"cmd":"state", "viewer":0}));
        let named = |id: &Value| mine["objects"][id.to_string()]["name"].clone();
        let bear = asked["among"].as_array().unwrap().iter().find(|o| named(o) == "Bear").unwrap_or_else(|| panic!("offered to its looker: {asked}")).clone();
        act(&mut say, 0, json!({"kind":"choose", "answer":{"objects":[bear]}}));
        let whole = say(json!({"cmd":"state"}));
        let hidden: Vec<&String> = whole["objects"].as_object().unwrap().iter().filter(|(_, o)| o.get("hidden").is_some()).map(|(id, _)| id).collect();
        assert_eq!(hidden.len(), 1);
        let card = hidden[0].clone();
        assert_eq!(whole["objects"][&card]["hidden"], json!({"lookers":[0], "card":{"name":"Bear"}}), "the harness sees all");
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!(theirs["objects"][&card]["name"], "");
        assert_eq!(theirs["objects"][&card]["types"], json!([]));
        assert_eq!(theirs["objects"][&card]["hidden"], json!({"lookers":[0], "card":null}));
        assert!(!theirs.to_string().contains("Bear"), "nothing names the card to the other seat: {theirs}");
        let mine = say(json!({"cmd":"state", "viewer":0}));
        assert_eq!(mine["objects"][&card]["name"], "");
        assert_eq!(mine["objects"][&card]["hidden"]["card"]["name"], "Bear");
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["objects"][&card]["hidden"]["card"]["name"], "Bear", "kept after observations were taken");
    }

    /// A manifested card through the pipe (701.40, 708.5, 708.9): every seat is shown the
    /// face-down 2/2 and only its controller the card; turning it face up is listed and
    /// submitted as a special action; and a manifested instant is revealed by name, the
    /// event and the disclosure agreeing.
    #[test]
    fn r708_5_face_down_permanents_through_the_pipe() {
        let defs = vec![
            CardDef::new("Summon", "{0}", "Sorcery", "Manifest the top card of your library.", None).unwrap(),
            CardDef::new("Opener", "{0}", "Instant", "Turn target face-down creature an opponent controls face up.", None).unwrap(),
            CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Glimpse", "{U}", "Instant", "Draw a card.", None).unwrap(),
            CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { game.put(p, 4, Zone::Library); } }
        game.put(0, 3, Zone::Library);
        game.put(0, 2, Zone::Library);
        let (first, second) = (game.put(0, 0, Zone::Hand), game.put(0, 0, Zone::Hand));
        let opener = game.put(1, 1, Zone::Hand);
        let forest = game.put(0, 4, Zone::Battlefield);
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        let act = |say: &mut dyn FnMut(Value) -> Value, player: usize, mv: Value| {
            let request = say(json!({"cmd":"state"}))["request_id"].clone();
            say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv}))
        };
        for (player, mv) in [(0, json!({"kind":"cast", "card":first.0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
            act(&mut say, player, mv);
        }
        let whole = say(json!({"cmd":"state"}));
        let down: Vec<&String> = whole["objects"].as_object().unwrap().iter().filter(|(_, o)| o.get("face_down").is_some()).map(|(id, _)| id).collect();
        assert_eq!(down.len(), 1);
        let bear = down[0].clone();
        assert_eq!(whole["objects"][&bear]["face_down"], json!({"by":"manifested", "card":{"name":"Bear"}}), "the harness sees all");
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!(theirs["objects"][&bear]["name"], "");
        assert_eq!(theirs["objects"][&bear]["face_down"], json!({"by":"manifested", "card":null}));
        assert_eq!(theirs["objects"][&bear]["power"], 2);
        assert!(!theirs.to_string().contains("Bear"), "nothing names the card to the other seat: {theirs}");
        let mine = say(json!({"cmd":"state", "viewer":0}));
        assert_eq!(mine["objects"][&bear]["name"], "");
        assert_eq!(mine["objects"][&bear]["face_down"], json!({"by":"manifested", "card":{"name":"Bear"}}));
        assert_eq!(mine["observations"][0]["kind"], "looked");
        assert_eq!(mine["observations"][0]["cards"][0]["name"], "Bear");
        // its controller's snapshot keeps the card after the observation was taken
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["objects"][&bear]["face_down"]["card"]["name"], "Bear");
        // The caller pays the manifest special action with the Forest (701.40b).
        act(&mut say, 0, json!({"kind":"turn_face_up","permanent":bear.parse::<u64>().unwrap(),"way":"manifested","mana":{"G":1},"plan":[{"source":forest.0,"ability":0}]}));
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!(theirs["objects"][&bear]["name"], "Bear");
        assert!(theirs["objects"][&bear].get("face_down").is_none());
        assert!(theirs["events"].as_array().unwrap().iter().any(|e| e["event"] == "turned_face_up" && e["object"].to_string() == bear));
        assert_eq!(theirs["observations"].as_array().unwrap().iter().map(|d| (d["kind"].clone(), d["by"].clone(), d["cards"][0]["name"].clone())).collect::<Vec<_>>(),
            vec![(json!("revealed"), json!(0), json!("Bear"))], "shown once, by its controller, as it was announced");
        assert_eq!(read_move(&json!({"kind":"turn_face_up", "permanent":1}), &Pool::default()), Err("a turn_face_up move says its \"way\": \"manifested\", \"cloaked\", \"morph\" or \"disguise\"".into()));
        // a manifested instant: revealed by its controller and left face down (701.40g)
        for (player, mv) in [(0, json!({"kind":"cast", "card":second.0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
            act(&mut say, player, mv);
        }
        let whole = say(json!({"cmd":"state"}));
        let glimpse = whole["objects"].as_object().unwrap().iter().find(|(_, o)| o.get("face_down").is_some()).map(|(id, _)| id.clone()).unwrap();
        say(json!({"cmd":"state", "viewer":0}));
        say(json!({"cmd":"state", "viewer":1}));
        let target = json!([[{"object": glimpse.parse::<u64>().unwrap()}]]);
        for (player, mv) in [(0, json!({"kind":"pass"})), (1, json!({"kind":"cast", "card":opener.0, "targets":target})), (1, json!({"kind":"pass"})), (0, json!({"kind":"pass"}))] {
            act(&mut say, player, mv);
        }
        for viewer in 0..2 {
            let seen = say(json!({"cmd":"state", "viewer":viewer}));
            let reveals: Vec<&Value> = seen["events"].as_array().unwrap().iter().filter(|e| e["event"] == "revealed").collect();
            assert_eq!(reveals.len(), 1, "seat {viewer}: {seen}");
            assert_eq!((&reveals[0]["player"], reveals[0]["cards"][0].to_string(), &reveals[0]["names"]), (&json!(0), glimpse.clone(), &json!(["Glimpse"])));
            let shown = seen["observations"].as_array().unwrap();
            assert_eq!(shown.len(), 1);
            assert_eq!((&shown[0]["kind"], &shown[0]["cards"][0]["name"], shown[0]["cards"][0]["object"].to_string()), (&json!("revealed"), &json!("Glimpse"), glimpse.clone()));
            assert_eq!(seen["objects"][&glimpse]["face_down"]["by"], "manifested", "still face down");
        }
    }

    /// A card cast face down with morph through the pipe (702.37c, 708.5): the face-down
    /// cast is listed with how it is face down and submitted as listed; on the stack and on
    /// the battlefield only its controller is shown the card; and turning it face up is listed
    /// and submitted with the morph way.
    #[test]
    fn r702_37_a_morph_spell_through_the_pipe() {
        let defs = vec![
            CardDef::new("Hidden Sage", "{7}{U}", "Creature — Human Wizard", "Morph {U}", Some((1, 1))).unwrap(),
            CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { game.put(p, 1, Zone::Library); } }
        let sage = game.put(0, 0, Zone::Hand);
        let islands: Vec<_> = (0..4).map(|_| game.put(0, 1, Zone::Battlefield)).collect();
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        let act = |say: &mut dyn FnMut(Value) -> Value, player: usize, mv: Value| {
            let request = say(json!({"cmd":"state"}))["request_id"].clone();
            say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv}))
        };
        act(&mut say, 0, json!({"kind":"cast","card":sage.0,"face_down":"morph","cost":{"alternative":0},"mana":{"U":3},"plan":islands[..3].iter().map(|o|json!({"source":o.0,"ability":0})).collect::<Vec<_>>()}));
        let whole = say(json!({"cmd":"state"}));
        let spell = whole["objects"].as_object().unwrap().iter().find(|(_, o)| o.get("face_down").is_some()).map(|(id, _)| id.clone()).expect("the face-down spell");
        assert_eq!(whole["objects"][&spell]["face_down"], json!({"by":"morph", "card":{"name":"Hidden Sage"}}));
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!((&theirs["objects"][&spell]["name"], &theirs["objects"][&spell]["face_down"]), (&json!(""), &json!({"by":"morph", "card":null})));
        assert!(!theirs.to_string().contains("Hidden Sage"), "nothing names the card to the other seat: {theirs}");
        let mine = say(json!({"cmd":"state", "viewer":0}));
        assert_eq!(mine["objects"][&spell]["face_down"]["card"]["name"], "Hidden Sage");
        act(&mut say, 0, json!({"kind":"pass"}));
        act(&mut say, 1, json!({"kind":"pass"}));
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert!(!theirs.to_string().contains("Hidden Sage"), "it entered face down: {theirs}");
        let permanent = say(json!({"cmd":"state"}))["objects"].as_object().unwrap().iter().find(|(_, o)| o.get("face_down").is_some() && o.get("tapped").is_some()).map(|(id, _)| id.clone()).expect("the face-down permanent");
        act(&mut say, 0, json!({"kind":"turn_face_up","permanent":permanent.parse::<u64>().unwrap(),"way":"morph","mana":{"U":1},"plan":[{"source":islands[3].0,"ability":0}]}));
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!(theirs["objects"][&permanent]["name"], "Hidden Sage");
        assert!(theirs["objects"][&permanent].get("face_down").is_none());
        assert!(matches!(read_move(&json!({"kind":"turn_face_up", "permanent":1, "way":"disguise"}), &Pool::default()).unwrap(), Move::TurnFaceUp(_, game::face::Way::Disguise)));
        // a Room's door, 0 the left (709.5e); there are only two
        assert!(matches!(read_move(&json!({"kind":"unlock", "room":1, "half":1}), &Pool::default()).unwrap(), Move::Unlock { half: 1, .. }));
        assert_eq!(read_move(&json!({"kind":"unlock", "room":1, "half":2}), &Pool::default()), Err("an unlock move says its \"half\": 0 or 1".into()));
        assert_eq!(read_move(&json!({"kind":"turn_face_up", "permanent":1, "way":"megamorph"}), &Pool::default()), Err("a turn_face_up move says its \"way\": \"manifested\", \"cloaked\", \"morph\" or \"disguise\"".into()));
    }

    /// A face-up permanent turned face down through the pipe (708.2a, 708.5): the event names
    /// it, how it is face down is public and only its controller is shown the card.
    #[test]
    fn r708_2a_a_permanent_turned_face_down_through_the_pipe() {
        let defs = vec![
            CardDef::new("Shy Sage", "{U}", "Creature — Human Wizard", "{0}: Turn this creature face down.\nMorph {U}", Some((1, 1))).unwrap(),
            CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut game = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..3 { game.put(p, 1, Zone::Library); } }
        let sage = game.put(0, 0, Zone::Battlefield);
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        let act = |say: &mut dyn FnMut(Value) -> Value, player: usize, mv: Value| {
            let request = say(json!({"cmd":"state"}))["request_id"].clone();
            say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv}))
        };
        say(json!({"cmd":"state", "viewer":1}));
        for (player, mv) in [(0, json!({"kind":"activate", "source":sage.0, "ability":0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
            act(&mut say, player, mv);
        }
        let id = sage.0.to_string();
        let theirs = say(json!({"cmd":"state", "viewer":1}));
        assert_eq!((&theirs["objects"][&id]["name"], &theirs["objects"][&id]["face_down"]), (&json!(""), &json!({"by":"turned face down", "card":null})));
        assert!(theirs["events"].as_array().unwrap().iter().any(|e| e["event"] == "turned_face_down" && e["object"].to_string() == id));
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["objects"][&id]["face_down"]["card"]["name"], "Shy Sage");
    }

    #[test]
    fn events_wait_for_each_viewer_as_rendered_when_they_happened_not_read_back_from_the_game() {
        let spell = CardDef::new("Free", "{0}", "Instant", "You gain 1 life.", None).unwrap();
        let mut game = Game::new(vec![spell], 2, 20);
        let card = game.put(0, 0, Zone::Hand);
        let mut simulation = Some(Simulation::new(game));
        let (mut session, mut logged, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut say = |input: Value| dispatch(&faces, &mut simulation, &mut session, &mut logged, &input).unwrap();
        for (player, mv) in [(0, json!({"kind":"cast", "card":card.0, "targets":[]})), (0, json!({"kind":"pass"})), (1, json!({"kind":"pass"}))] {
            let request = say(json!({"cmd":"state", "viewer":0}))["request_id"].clone();
            say(json!({"cmd":"act", "session_id":1, "request_id":request, "player":player, "move":mv, "viewer":0}));
        }
        // seat 1 asks for the first time only now, and is told all of it, once
        let late = say(json!({"cmd":"state", "viewer":1}));
        let kinds: Vec<&str> = late["events"].as_array().unwrap().iter().map(|e| e["event"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"life_gained"), "{kinds:?}");
        assert_eq!(say(json!({"cmd":"state", "viewer":1}))["events"], json!([]));
        // seat 0 was answered after each command: nothing is left for it, and the harness
        // view has its own copy with the engine's own log beside it
        assert_eq!(say(json!({"cmd":"state", "viewer":0}))["events"], json!([]));
        let whole = say(json!({"cmd":"state"}));
        assert_eq!(whole["events"].as_array().unwrap().len(), late["events"].as_array().unwrap().len());
        assert_eq!(whole["log"].as_array().unwrap().len(), whole["events"].as_array().unwrap().len());
        assert_eq!(say(json!({"cmd":"state"}))["log"], json!([]));
    }

    #[test]
    fn a_visibility_query_failure_is_contained_at_the_observation_boundary() {
        let mut malformed = Game::new(vec![], 2, 20);
        // Controlled corrupt event: legal offers do not inspect it, but seat visibility
        // must query the referenced object while rendering the observation.
        malformed.log.push(Event::Drew { player: 0, card: ObjId(u64::MAX), ordinal: 1, in_draw_step: None });
        let mut simulation = Some(Simulation::new(malformed));
        let result = dispatch(&Catalog::new(BTreeMap::new()), &mut simulation, &mut 1, &mut Delivery::default(),
            &json!({"cmd":"state", "viewer":0}));
        let refusal=result.unwrap_err();
        assert!(refusal.starts_with("simulation fault at Observation request "), "{refusal}");
        let failed = simulation.as_ref().unwrap();
        let fault=failed.fault().unwrap();
        assert_eq!(fault.stage, game::simulation::Stage::Observation);
        assert_eq!(&failed.game().unwrap_err(), fault, "faulted state cannot resume outside containment");
    }

    #[test]
    fn correlated_requests_preserve_seat_privacy() {
        let secret = CardDef::new("Secret", "{0}", "Instant", "You gain 1 life.", None).unwrap();
        let mut game = Game::new(vec![secret], 2, 20);
        let card = game.put(0, 0, Zone::Hand);
        let mut simulation = Some(Simulation::new(game));
        let mut session = 1;
        let mut logged = Delivery::default();
        let faces = Catalog::new(BTreeMap::new());
        let own = dispatch(&faces, &mut simulation, &mut session, &mut logged, &json!({"cmd":"state","viewer":0})).unwrap();
        let other = dispatch(&faces, &mut simulation, &mut session, &mut logged, &json!({"cmd":"state","viewer":1})).unwrap();
        assert_eq!(own["request_id"], other["request_id"]);
        assert_eq!(other["asked"], json!({"kind":"priority", "player":0, "request_id":own["request_id"]}));
        assert!(other["players"][0].get("hand").is_none());
        assert!(other["objects"].get(card.0.to_string()).is_none());
        assert!(own["objects"].get(card.0.to_string()).is_some());
    }
}

#[test]
fn r121_2_the_pipe_offers_player_draw_substitutions_and_keeps_instance_identity() {
    let defs = vec![
        game::CardDef::new("Dredger", "{0}", "Creature", "Dredge 1, dredge 2", Some((1, 1))).unwrap(),
        game::CardDef::new("Blank", "", "Land", "", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    let source = g.put(0, 0, Zone::Graveyard);
    let library:Vec<_>=(0..4).map(|_|g.put(0,1,Zone::Library)).collect();
    g.draw(0, 2);
    let shown = state(&mut g, Some(0), &mut Delivery::default());
    let asked = &shown["asked"];
    assert_eq!(asked["on"], json!({"player": 0}));
    assert_eq!(asked["event"], "draw");
    assert_eq!(asked["optional"], true);
    assert_eq!(asked["among"][1]["part"], 1);
    assert_eq!(asked["among"][1]["source"], source.0);
    let answer = json!({"kind":"choose", "answer":{"replacement":asked["among"][1]}});
    g.act(0, read_move(&answer, &Pool::default()).unwrap()).unwrap();
    // Dredge mills together; the owner orders the two cards before the return/draw resume.
    g.act(0,read_move(&json!({"kind":"choose","answer":{"objects":[library[3].0,library[2].0]}}),&Pool::default()).unwrap()).unwrap();
    assert_eq!(g.log.iter().filter(|e| matches!(e, game::Event::Milled { .. })).count(), 2);
    assert_eq!(g.log.iter().filter(|e| matches!(e, game::Event::Drew { .. })).count(), 1);
    assert!(g.decision.is_none());
}

#[test]
fn r114_public_emblem_zone_and_ability_text_do_not_expose_hidden_cards() {
    let maker = CardDef::new("Maker", "{0}", "Sorcery", "You get an emblem with \"Whenever you cast a spell, you gain 1 life.\"", None).unwrap();
    let mut g = Game::new(vec![maker], 2, 20);
    let secret = g.put(0, 0, Zone::Hand);
    let card = g.put(0, 0, Zone::Hand);
    g.act(0, Move::Cast { card, targets: vec![] }).unwrap();
    for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
    let emblem = g.command[0];
    for viewer in 0..2 {
        let shown = state(&mut g, Some(viewer), &mut Delivery::default());
        assert_eq!(shown["command"], json!([emblem.0]));
        let object = &shown["objects"][emblem.0.to_string()];
        assert_eq!(object["emblem"], true);
        assert_eq!(object["token"], false);
        assert!(object["abilities"][0].as_str().unwrap().contains("cast a spell"));
        assert_eq!(shown["objects"].get(secret.0.to_string()).is_some(), viewer == 0);
    }
}

#[test]
fn r702_21a_a_ward_life_payment_is_asked_and_explicitly_paid() {
    let defs = vec![
        CardDef::new("Warded", "{0}", "Creature — Bear", "Ward—Pay 7 life.", Some((2, 2))).unwrap(),
        CardDef::new("Shock", "{0}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
    ];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let warded = g.put(0, 0, Zone::Battlefield);
    g.act(0, Move::Pass).unwrap();
    let shock = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: shock, targets: vec![vec![Target::Object(warded)]] }).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let question = asked(&mut g);
    assert_eq!((question["kind"].clone(), question["life"].clone()), (json!("pay"), json!(7)));
    g.act(1, read_move(&json!({"kind": "choose", "answer": {"pay":{"plan":[]}}}), &Pool::default()).unwrap()).unwrap();
    assert_eq!(g.players[1].life, 13);
}

#[test]
fn r702_21a_a_ward_discard_is_asked_as_a_may_naming_the_discard_then_which_card() {
    let defs = vec![
        CardDef::new("Warded", "{0}", "Creature — Bear", "Ward—Discard a card.", Some((2, 2))).unwrap(),
        CardDef::new("Shock", "{0}", "Instant", "Shock deals 2 damage to any target.", None).unwrap(),
    ];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let warded = g.put(0, 0, Zone::Battlefield);
    let fodder = [g.put(1, 0, Zone::Hand), g.put(1, 1, Zone::Hand)];
    g.act(0, Move::Pass).unwrap();
    let shock = g.put(1, 1, Zone::Hand);
    g.act(1, Move::Cast { card: shock, targets: vec![vec![Target::Object(warded)]] }).unwrap();
    while g.decision.is_none() { g.act(g.priority, Move::Pass).unwrap(); }
    let question = asked(&mut g);
    assert_eq!(question["kind"], json!("may"));
    // the question is for the ward trigger, whose text names the cost
    let prompt = Simulation::new(g.clone()).prompt(1).unwrap();
    let occurrence = prompt.decision.as_ref().and_then(|d| d.occurrence.as_ref()).expect("asked for an occurrence");
    let said = occurrence_json(occurrence);
    assert_eq!(said["source"], json!(warded.0));
    assert!(said["text"].as_str().unwrap().contains("unless its controller discards a card"), "{said}");
    g.act(1, read_move(&json!({"kind": "choose", "answer": "yes"}), &Pool::default()).unwrap()).unwrap();
    let question = asked(&mut g);
    assert_eq!((question["kind"].clone(), question["among"].clone()), (json!("objects"), json!([fodder[0].0, fodder[1].0])));
    g.act(1, read_move(&json!({"kind": "choose", "answer": {"objects": [fodder[1].0]}}), &Pool::default()).unwrap()).unwrap();
    assert_eq!(g.players[1].hand, vec![fodder[0]]);
}

#[test]
fn r702_174a_a_gift_listing_is_sent_back_as_the_cast_that_promises_it() {
    let defs = vec![
        CardDef::new("Sazacap's Brew", "{1}{R}", "Instant", "Gift a tapped Fish\nAs an additional cost to cast this spell, discard a card.\nTarget player draws two cards. If the gift was promised, target creature you control gets +2/+0 until end of turn.", None).unwrap(),
        CardDef::new("Grizzly Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
    ];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    for p in 0..2 { for _ in 0..3 { g.put(p, 1, Zone::Library); } }
    let brew = g.put(0, 0, Zone::Hand);
    let fodder = g.put(0, 1, Zone::Hand);
    let bears = g.put(0, 1, Zone::Battlefield);
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    g.add_mana(0, ir::mana::ManaSymbol::Colorless);
    let command=json!({"kind":"cast","card":brew.0,"mana":{"C":1,"R":1},"gift":1,"targets":[[{"player":0}],[{"object":bears.0}]],"paying":[[fodder.0]]});
    g.act(0, read_move(&command, &Pool::default()).unwrap()).unwrap();
    assert_eq!(g.obj(g.stack[0]).gift, Some(1));
    // "gift" is said with a cast only, and of a player by number
    assert_eq!(read_move(&json!({"kind": "pass", "gift": 1}), &Pool::default()), Err("a pass move has no \"gift\"; it says nothing more".into()));
    assert_eq!(read_move(&json!({"kind": "cast", "card": brew.0, "gift": "them"}), &Pool::default()), Err("a gift is promised to an opponent, by player label (\"p1\"), not \"them\"".into()));
}

#[test]
fn r702_35_a_madness_offer_is_answered_with_its_supplied_cost() {
    let defs = vec![
        CardDef::new("Fiery Temper", "{1}{R}{R}", "Instant", "Fiery Temper deals 3 damage to any target.\nMadness {R}", None).unwrap(),
        CardDef::new("Rummage", "{0}", "Instant", "Discard a card.", None).unwrap(),
    ];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    g.put(0, 0, Zone::Hand);
    let rummage = g.put(0, 1, Zone::Hand);
    g.act(0, Move::Cast { card: rummage, targets: vec![] }).unwrap();
    for _ in 0..4 { g.act(g.priority, Move::Pass).unwrap(); }
    g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
    // The madness trigger offers only the card identity; its printed madness row is supplied.
    let offer = asked(&mut g);
    assert_eq!(offer["kind"], "cast");
    let card = g.exile[0];
    assert_eq!(offer["cards"], json!([card.0]));
    let command = json!({"kind":"cast","card":card.0,"cost":{"alternative":1},"mana":{"R":1},"targets":[[{"player":1}]]});
    // the same option said without its cost is its mana cost, which the offer does not allow
    let mut without = command.clone();
    without.as_object_mut().unwrap().remove("cost");
    let retained=format!("{g:?}");
    assert_eq!(g.act(0, read_move(&without, &Pool::default()).unwrap()), Err("the effect lets that card be cast only for the cost it offers (118.9a)".into()));
    assert_eq!(format!("{g:?}"),retained);
    g.act(0, read_move(&command, &Pool::default()).unwrap()).unwrap();
    assert_eq!(g.stack.len(), 1);
    assert_eq!(g.obj(g.stack[0]).chars.name, "Fiery Temper");
}

#[test]
fn r118_9_an_alternative_cost_option_is_sent_back_as_the_move_that_casts_for_it() {
    let defs = vec![
        CardDef::new("Fireblast", "{4}{R}{R}", "Instant", "You may sacrifice two Mountains rather than pay this spell's mana cost.\nFireblast deals 4 damage to any target.", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "({T}: Add {R}.)", None).unwrap(),
    ];
    for d in &defs { check(d).unwrap(); }
    let mut g = Game::new(defs, 2, 20);
    let blast = g.put(0, 0, Zone::Hand);
    let mountains = [g.put(0, 1, Zone::Battlefield), g.put(0, 1, Zone::Battlefield)];
    let command=json!({"kind":"cast","card":blast.0,"cost":{"alternative":0},"targets":[[{"player":1}]],"paying":[[mountains[0].0,mountains[1].0]]});
    g.act(0, read_move(&command, &Pool::default()).unwrap()).unwrap();
    // The cost's simultaneous sacrifice waits for the owner's graveyard order (404.3).
    g.act(0,read_move(&json!({"kind":"choose","answer":{"objects":[mountains[0].0,mountains[1].0]}}),&Pool::default()).unwrap()).unwrap();
    assert!(mountains.iter().all(|m| !g.battlefield.contains(m)), "sacrificed as it was cast");
    assert_eq!(g.stack.len(), 1);
    // the mana cost said outright is the default; another word is refused
    assert!(matches!(read_move(&json!({"kind":"cast", "card": blast.0, "cost": "mana"}), &Pool::default()).unwrap(), Move::CastFor(game::announcement::CastCost::Mana, _)));
    assert_eq!(read_move(&json!({"kind":"cast", "card": blast.0, "cost": "free"}), &Pool::default()), Err("a cost is \"mana\", {\"alternative\": an ability row}, or {\"flashback\": a cost index}".into()));
    assert_eq!(read_move(&json!({"kind":"pass", "cost": {"alternative": 0}}), &Pool::default()), Err("a pass move has no \"cost\"; it says nothing more".into()));
}

#[test]
fn r700_3_multiplayer_pile_choices_round_trip_through_the_json_adapter() {
    let faces = corpus::load(&corpus::default_path()).unwrap();
    let catalog = CardDef::catalog(&faces);
    let jace = CardDef::from_faces(catalog.get("Jace, Architect of Thought").unwrap()).unwrap();
    let land = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
    let mut g = Game::new(vec![jace, land], 3, 20);
    let cards: Vec<_> = (0..3).map(|_| g.put(0, 1, Zone::Library)).collect();
    let source = g.put(0, 0, Zone::Battlefield);
    g.put_counters(source, "loyalty", 4);
    g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
    for _ in 0..3 { g.act(g.priority, Move::Pass).unwrap(); }
    let mut delivery = Delivery::default();
    let asked = state(&mut g, Some(0), &mut delivery)["asked"].clone();
    assert_eq!(asked["kind"], "player");
    assert_eq!(asked["among"], json!([1, 2]));
    let choose = read_move(&json!({"kind":"choose", "answer":{"player":2}}), &Pool::default()).unwrap();
    let retained=format!("{g:?}");
    assert_eq!(g.act(1, choose.clone()), Err("that decision is another player's".into()));
    assert_eq!(format!("{g:?}"),retained);
    g.act(0, choose).unwrap();
    let asked = state(&mut g, Some(2), &mut delivery)["asked"].clone();
    assert_eq!(asked["kind"], "select");
    g.act(2, read_move(&json!({"kind":"choose", "answer":{"objects":[cards[0].0]}}), &Pool::default()).unwrap()).unwrap();
    let asked = state(&mut g, Some(0), &mut delivery)["asked"].clone();
    assert_eq!(asked["kind"], "pile");
    assert_eq!(asked["piles"][0], json!([cards[0].0]));
    assert_eq!(asked["piles"][1].as_array().unwrap().len(), 2);
    let retained=format!("{g:?}");
    assert_eq!(g.act(0, read_move(&json!({"kind":"choose", "answer":{"pile":2}}), &Pool::default()).unwrap()), Err("choose one of the piles (700.3)".into()));
    assert_eq!(format!("{g:?}"),retained);
    g.act(0, read_move(&json!({"kind":"choose", "answer":{"pile":0}}), &Pool::default()).unwrap()).unwrap();
    // The other pile's bottom order is still a real decision of the controller.
    let asked = state(&mut g, Some(0), &mut delivery)["asked"].clone();
    assert_eq!(asked["kind"], "order_cards");
    assert_eq!((asked["player"].clone(),asked["to"].clone()),
        (json!(0),json!({"zone":"library","position":"bottom"})));
    let among=asked["cards"].as_array().unwrap();
    assert_eq!(among.len(),2);
    assert!(among.contains(&json!(cards[1].0)) && among.contains(&json!(cards[2].0)));
    // The caller authors this top-first order among the two identical physical Islands.
    g.act(0, read_move(&json!({"kind":"choose", "answer":{"objects":[cards[1].0,cards[2].0]}}), &Pool::default()).unwrap()).unwrap();
    assert_eq!(g.players[0].hand.len(), 1);
    assert_eq!(g.players[0].library.len(), 2);
    assert_eq!(g.players[0].library.iter().rev().map(|o|g.physical_card_identity(*o)).collect::<Vec<_>>(),
        vec![g.physical_card_identity(cards[1]),g.physical_card_identity(cards[2])]);
    assert!(g.stack.is_empty());
}

#[test]
fn independent_setups_share_only_the_immutable_catalog() {
    use std::sync::Arc;
    let faces = corpus::load(&corpus::default_path()).unwrap();
    let catalog = Catalog::new(CardDef::catalog(&faces));
    let request = json!({"players":[{"hand":["Forest"]},{}]});
    let mut first = new_game(&catalog, &request).unwrap();
    let second = new_game(&catalog, &request).unwrap();
    assert!(Arc::ptr_eq(&first.defs, &second.defs));
    first.players[0].life = 1;
    first.put(0, 0, Zone::Battlefield);
    assert_eq!(second.players[0].life, 20);
    assert!(second.battlefield.is_empty());
    let other = new_game(&catalog, &json!({"players":[{"hand":["Island"]},{}]})).unwrap();
    assert!(!Arc::ptr_eq(&first.defs, &other.defs));
    assert_eq!(second.defs[0].name(), "Forest");
    assert_eq!(other.defs[0].name(), "Island");
    assert_eq!(new_game(&catalog, &json!({"players":[{"hand":["not a real card"]},{}]})).unwrap_err(), "unknown card: not a real card");
    let again = new_game(&catalog, &json!({"players":[{"hand":["Island"]},{}]})).unwrap();
    assert!(Arc::ptr_eq(&other.defs, &again.defs), "failed setups do not replace a valid catalog");
    let decks = json!({"decks":[["Island"],["Island"]]});
    let started = start_game(&catalog, &decks).unwrap();
    assert!(Arc::ptr_eq(&other.defs, &started.defs));
}

#[test]
fn r608_2d_alternative_action_question_round_trips_to_the_retained_procedure() {
    let defs = vec![
        CardDef::new("Choice", "{0}", "Sorcery", "You may discard a card or sacrifice a land. If you do, draw two cards.", None).unwrap(),
        CardDef::new("Forest", "", "Basic Land — Forest", "", None).unwrap(),
    ];
    let mut g = Game::new(defs, 2, 20);
    for _ in 0..3 { g.put(0,1,Zone::Library); }
    let land=g.put(0,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();g.act(0,Move::Pass).unwrap();g.act(1,Move::Pass).unwrap();
    let question=asked(&mut g);
    assert_eq!(question["kind"],"alternative");assert_eq!(question["among"],json!([{"index":1,"action":"Sacrifice","among":[land.0],"count":1}]));
    let command=json!({"kind":"choose","answer":{"alternative":1}});
    let mv=read_move(&command,&Pool::default()).unwrap();assert_eq!(answered(&mv),command);
    g.act(0,mv).unwrap();assert!(!g.battlefield.contains(&land));assert_eq!(g.players[0].hand.len(),2);
}

#[test]
fn r702_170_plot_option_and_public_designation_round_trip() {
    let defs = vec![
        CardDef::new("Plottable", "{4}{R}", "Sorcery", "You gain 1 life.\nPlot {1}{R}", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    let card=g.put(0,0,Zone::Hand);
    let a=g.put(0,1,Zone::Battlefield); let b=g.put(0,1,Zone::Battlefield);
    let option=json!({"kind":"plot","card":card.0,"mana":{"R":2},"plan":[{"source":a.0,"ability":0},{"source":b.0,"ability":0}]});
    g.act(0,read_move(&option,&Pool::default()).unwrap()).unwrap();
    let exiled=g.exile[0];
    for player in 0..2 {
        let shown=state(&mut g,Some(player),&mut Delivery::default());
        assert_eq!(shown["objects"][exiled.0.to_string()]["plotted"],json!({"turn":g.turn}));
    }
    assert!(g.stack.is_empty());
}

#[test]
fn r702_180_harmonize_routes_name_the_creature_to_tap_and_round_trip() {
    let defs = vec![
        CardDef::new("Chorus", "{2}{U}", "Sorcery", "You gain 1 life.\nHarmonize {4}{U}", None).unwrap(),
        CardDef::new("Bear", "{0}", "Creature — Bear", "", Some((3, 2))).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card=g.put(0,0,Zone::Graveyard);
    let bear=g.put(0,1,Zone::Battlefield);
    g.players[0].pool.colors[1]=5;
    let option=json!({"kind":"cast","card":card.0,"mana":{"U":2},"route":{"harmonize":bear.0}});
    let mv=read_move(&option,&Pool::default()).unwrap();
    assert!(format!("{mv:?}").contains("Harmonize"),"{mv:?}");
    assert_eq!(read_move(&json!({"kind":"cast","card":card.0,"route":{"harmonize":"bear"}}),&Pool::default()), Err("an object label, not \"bear\"".into()));
    g.act(0,mv).unwrap();
    assert!(g.obj(bear).tapped);
    assert_eq!(g.players[0].pool.total(),3,"{{1}}{{U}} of the five");
}

#[test]
fn r702_185_a_warped_card_is_a_public_designation() {
    let defs = vec![CardDef::new("Warper", "{3}{G}", "Creature — Beast", "Warp {G}", Some((2, 2))).unwrap()];
    let mut g=Game::new(defs,2,20);
    for p in 0..2 { for _ in 0..5 { g.put(p,0,Zone::Library); } }
    while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
    let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[4]=1;
    let option=json!({"kind":"cast","card":card.0,"mana":{"G":1},"cost":{"alternative":0}});
    g.act(0,read_move(&option,&Pool::default()).unwrap()).unwrap();
    let turn=g.turn;
    while g.exile.is_empty() || !g.stack.is_empty() {
        match g.decision { Some(game::state::Decision::Attackers{player})=>g.act(player,Move::Attack(vec![])).unwrap(), _=>g.act(g.priority,Move::Pass).unwrap() }
    }
    assert_eq!(g.turn,turn);
    let exiled=g.exile[0];
    for player in 0..2 {
        let shown=state(&mut g,Some(player),&mut Delivery::default());
        assert_eq!(shown["objects"][exiled.0.to_string()]["warped"],json!({"turn":turn}));
    }
}

#[test]
fn r702_143_foretell_and_foretold_cast_round_trip_without_naming_the_card() {
    let defs = vec![
        CardDef::new("Foretold shock", "{2}{R}", "Instant", "Foretold shock deals 2 damage to any target.\nForetell {R}", None).unwrap(),
        CardDef::new("Mountain", "", "Basic Land — Mountain", "", None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    for p in 0..2 { for _ in 0..5 { g.put(p,1,Zone::Library); } }
    let card=g.put(0,0,Zone::Hand);
    let mountains: Vec<_> = (0..3).map(|_|g.put(0,1,Zone::Battlefield)).collect();
    let mut seats=[Delivery::default(),Delivery::default()];
    let option=json!({"kind":"foretell","card":card.0,"mana":{"R":2},"plan":[{"source":mountains[0].0,"ability":0},{"source":mountains[1].0,"ability":0}]});
    g.act(0,read_move(&option,&Pool::default()).unwrap()).unwrap();
    let exiled=g.exile[0];
    let turn=g.turn;
    for (player,delivery) in seats.iter_mut().enumerate() {
        let shown=state(&mut g,Some(player),delivery);
        let o=&shown["objects"][exiled.0.to_string()];
        assert_eq!(o["foretold"],json!({"turn":turn}));
        assert_eq!(o["hidden"]["lookers"],json!([0]));
        assert_eq!(o["hidden"]["card"],if player==0 {json!({"name":"Foretold shock"})} else {json!(null)});
        assert_eq!(o["name"],json!(""));
        assert!(shown["events"].as_array().unwrap().iter().any(|e|e["event"]=="foretold"&&e["card"]==json!(exiled.0)),"{}",shown["events"]);
        if player==1 { assert!(!shown.to_string().contains("Foretold shock"),"seat 1 is not told the foretold card"); }
    }
    let mv=json!({"kind":"cast","card":exiled.0,"route":"foretell","mana":{"R":1},"targets":[[{"player":1}]],"plan":[{"source":mountains[2].0,"ability":0}]});
    let mut same_turn=g.clone();
    assert_eq!(same_turn.act(0,read_move(&mv,&Pool::default()).unwrap()), Err("only a card you foretold is cast for its foretell cost, after the turn it was foretold (702.143a)".into()));
    assert_eq!(same_turn.exile,vec![exiled]);
    assert!(same_turn.stack.is_empty() && !same_turn.obj(mountains[2]).tapped);
    assert_eq!(same_turn.players[0].pool.total(),0);
    assert!(same_turn.hidden(exiled).is_some());
    while g.active==0 {
        let action=if matches!(g.decision,Some(Decision::Attackers{..})) { Move::Attack(vec![]) } else { Move::Pass };
        g.act(g.priority,action).unwrap();
    }
    while g.priority!=0 { g.act(g.priority,Move::Pass).unwrap(); }
    g.act(0,read_move(&mv,&Pool::default()).unwrap()).unwrap();
    while !g.stack.is_empty() { g.act(g.priority,Move::Pass).unwrap(); }
    assert_eq!(g.players[1].life,18);
}

#[test]
fn r702_122_crew_validates_supplied_total_power_and_tells_both_events() {
    let defs = vec![
        CardDef::new("Copter", "{2}", "Artifact — Vehicle", "Crew 3", Some((3, 3))).unwrap(),
        CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
        CardDef::new("Wall", "{0}", "Creature — Wall", "", Some((0, 4))).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    for p in 0..2 { for _ in 0..5 { g.put(p,2,Zone::Library); } }
    let copter=g.put(0,0,Zone::Battlefield);
    let (a,b,wall)=(g.put(0,1,Zone::Battlefield),g.put(0,1,Zone::Battlefield),g.put(0,2,Zone::Battlefield));
    // The one printed Crew keyword row is index0; its derived activated meaning is index1.
    let ability = 1;
    assert_eq!(g.abilities(copter).len(),2);
    assert!(matches!(&*g.abilities(copter)[ability],ir::Ability::Activated { .. }));
    let mut mv=json!({"kind":"activate","source":copter.0,"ability":ability,"paying":[[a.0]],"targets":[]});
    let retained=format!("{g:?}");
    assert_eq!(g.act(0,read_move(&mv,&Pool::default()).unwrap()), Err("that selection does not satisfy the cost: the chosen creatures have total power less than 3 (601.2h)".into()));
    assert_eq!(format!("{g:?}"),retained);
    mv["paying"]=json!([[a.0,b.0,wall.0]]);
    g.act(0,read_move(&mv,&Pool::default()).unwrap()).unwrap();
    while !g.stack.is_empty() { g.act(g.priority,Move::Pass).unwrap(); }
    let mut delivery=Delivery::default();
    let shown=state(&mut g,Some(1),&mut delivery);
    let events=shown["events"].as_array().unwrap();
    assert!(events.iter().any(|e|e["event"]=="crews"&&e["vehicle"]==json!(copter.0)&&e["creatures"].as_array().unwrap().len()==3),"{events:?}");
    assert!(events.iter().any(|e|e["event"]=="became_crewed"&&e["vehicle"]==json!(copter.0)),"{events:?}");
    assert!(g.is_a(copter,ir::CardType::Creature));
}

#[test]
fn r725_2_monarch_and_source_free_trigger_ordering_are_visible_through_the_pipe() {
    let defs=vec![
        CardDef::new("Watcher","{0}","Enchantment","At the beginning of your end step, you gain 1 life.",None).unwrap(),
        CardDef::new("Blank","","Land","",None).unwrap(),
        CardDef::new("Stifle","{0}","Instant","Counter target activated or triggered ability.",None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);g.monarch=Some(0);g.put(0,0,Zone::Battlefield);
    for p in 0..2 {for _ in 0..12 {g.put(p,1,Zone::Library);}}
    let counter=g.put(0,2,Zone::Hand);
    while g.part!=ir::Part::End {g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    assert_eq!(shown["monarch"],0);assert_eq!(shown["asked"]["kind"],"order");
    let abilities=shown["asked"]["abilities"].as_array().unwrap();
    let rule=abilities.iter().find(|a|a["source"].is_null()).unwrap();
    assert_eq!(rule["origin"],"rule");assert!(rule["text"].as_str().unwrap().contains("draw"));
    g.act(0,read_move(&json!({"kind":"order","order":[0,1]}),&Pool::default()).unwrap()).unwrap();
    let ability=*g.stack.iter().find(|id|g.obj(**id).ability==Some((None,0))).unwrap();
    g.act(0,Move::Cast{card:counter,targets:vec![vec![Target::Object(ability)]]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    assert!(!g.stack.contains(&ability));
    assert_eq!(g.monarch,Some(0));
    assert_eq!(g.players[0].hand.len(),0);
}

#[test]
fn r702_179_speed_and_its_source_free_rule_are_visible_through_the_pipe() {
    let defs=vec![
        CardDef::new("Engine","{0}","Artifact","Start your engines!",None).unwrap(),
        CardDef::new("Blank","","Land","",None).unwrap(),
        CardDef::new("Drain","{0}","Instant","Target player loses 1 life.",None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    for p in 0..2 {for _ in 0..12 {g.put(p,1,Zone::Library);}}
    while g.part!=ir::Part::PrecombatMain {g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    assert!(shown["players"][0].get("speed").is_none(),"no speed until it is set (702.179b)");
    g.put(0,0,Zone::Battlefield);
    let drain=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card:drain,targets:vec![vec![Target::Player(1)]]}).unwrap();
    let mut delivery=Delivery::default();
    let shown=state(&mut g,Some(0),&mut delivery);
    assert_eq!(shown["players"][0]["speed"],1);
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    // the speed rule triggered: an ability with no source, of the rules
    let ability=*g.stack.iter().find(|id|g.obj(**id).ability.is_some_and(|(source,_)|source.is_none())).expect("the speed rule on the stack");
    let shown=state(&mut g,Some(0),&mut delivery);
    let on_stack=&shown["objects"][ability.0.to_string()];
    // "This ability has no source" (702.179d): no source's name either
    assert_eq!((on_stack["name"].as_str(),on_stack["ability"].as_bool(),on_stack.get("source")),(Some(""),Some(true),None),"{on_stack}");
    // a prompt says its text, as of a card's ability (702.179d)
    let prompt=Simulation::new(g.clone()).prompt(0).unwrap();
    let text=prompt.entities[&ability].occurrence.as_ref().and_then(|o|o.text.clone()).unwrap_or_default();
    assert!(text.contains("your speed increases by 1")&&text.contains("only once each turn"),"{text}");
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(0),&mut delivery);
    assert_eq!(shown["players"][0]["speed"],2);
    assert!(shown["events"].as_array().unwrap().iter().any(|e|e["event"]=="speed"&&e["player"]==0&&e["speed"]==2&&e["previous"]==1),"{}",shown["events"]);
}

#[test]
fn r701_37b_monstrous_is_shown_on_the_permanent_with_its_event() {
    let defs=vec![
        CardDef::new("Beast","{0}","Creature — Beast","{0}: Monstrosity 2.",Some((2,2))).unwrap(),
        CardDef::new("Blank","","Land","",None).unwrap(),
    ];
    for d in &defs {check(d).unwrap();}
    let mut g=Game::new(defs,2,20);
    for p in 0..2 {for _ in 0..5 {g.put(p,1,Zone::Library);}}
    while g.part!=ir::Part::PrecombatMain {g.act(g.priority,Move::Pass).unwrap();}
    let beast=g.put(0,0,Zone::Battlefield);
    let mut delivery=Delivery::default();
    let shown=state(&mut g,Some(1),&mut delivery);
    assert!(shown["objects"][beast.0.to_string()].get("monstrous").is_none());
    g.act(0,Move::Activate{source:beast,ability:0,targets:vec![],paying:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(1),&mut delivery);
    assert_eq!(shown["objects"][beast.0.to_string()]["monstrous"],true);
    assert!(shown["events"].as_array().unwrap().iter().any(|e|e["event"]=="designated"&&e["designation"]=="monstrous"),"{}",shown["events"]);
}

#[test]
fn r702_81_casting_routes_round_trip_with_the_discard_selection() {
    let defs=vec![
        CardDef::new("Retrace witness","{0}","Sorcery","You gain 1 life.\nRetrace\nFlashback {0}",None).unwrap(),
        CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
    ];
    for d in &defs {check(d).unwrap();}
    let mut g=Game::new(defs,2,20);
    let card=g.put(0,0,Zone::Graveyard);
    let land=g.put(0,1,Zone::Hand);
    let flashback = json!({"kind":"cast","card":card.0,"route":"flashback","targets":[]});
    g.clone().act(0, read_move(&flashback, &Pool::default()).unwrap()).unwrap();
    let command=json!({"kind":"cast","card":card.0,"route":"retrace","targets":[],"paying":[[land.0]]});
    g.act(0,read_move(&command,&Pool::default()).unwrap()).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    assert_eq!(g.players[0].life,21);
    assert!(g.exile.is_empty());
    assert_eq!(g.players[0].graveyard.len(),2);
    for route in [json!("unknown"),json!(7),json!(["retrace","flashback"])] {
        assert_eq!(read_move(&json!({"kind":"cast","card":card.0,"route":route}),&Pool::default()), Err("a casting route is normal, flashback, retrace, escape, foretell, harmonize ({\"harmonize\": a creature to tap}) or a permission source and ability".into()));
    }
    assert_eq!(read_move(&json!({"kind":"pass","route":"retrace"}),&Pool::default()), Err("a pass move has no \"route\"; it says nothing more".into()));
}

#[test]
fn r702_66_delve_count_and_selected_exiles_round_trip_through_the_pipe() {
    let definitions=vec![CardDef::new("Delve witness","{2}{U}","Sorcery","Delve\nYou gain 1 life.",None).unwrap(),CardDef::new("Land","","Land","",None).unwrap()];
    let mut g=Game::new(definitions,2,20);let card=g.put(0,0,Zone::Hand);let a=g.put(0,1,Zone::Graveyard);let b=g.put(0,1,Zone::Graveyard);g.players[0].pool.colors[1]=1;
    let selected=json!({"kind":"cast","card":card.0,"delve":2,"mana":{"U":1},"paying":[[a.0,b.0]],"targets":[]});
    g.act(0,read_move(&selected,&Pool::default()).unwrap()).unwrap();assert_eq!(g.exile.len(),2);
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}assert_eq!(g.players[0].life,21);
    for count in [json!(-1),json!("two"),json!(4294967296u64)]{assert_eq!(read_move(&json!({"kind":"cast","card":card.0,"delve":count}),&Pool::default()), Err("delve is a nonnegative card count".into()));}
    assert_eq!(read_move(&json!({"kind":"pass","delve":1}),&Pool::default()), Err("a pass move has no \"delve\"; it says nothing more".into()));
}

#[test]
fn r702_49_ninjutsu_hand_offer_cost_and_reveal_lifetime_use_normal_wire_commands() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let kaito=CardDef::from_face(faces.iter().find(|f|f.face_name=="Kaito, Bane of Nightmares").unwrap()).unwrap();
    let bear=CardDef::new("Bear","{0}","Creature — Bear","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![kaito,bear],2,20);
    let attacker=g.put(0,1,Zone::Battlefield);let card=g.put(0,0,Zone::Hand);
    for _ in 0..4 {g.act(g.priority,Move::Pass).unwrap();}
    g.act(0,Move::Attack(vec![(attacker,Target::Player(1))])).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    if matches!(g.decision,Some(game::state::Decision::Blockers{..})) {g.act(1,Move::Block(vec![])).unwrap();}g.players[0].pool.colors=[0,1,1,0,0];g.players[0].pool.colorless=1;
    let mut delivery=Delivery::default();
    let hidden=state(&mut g,Some(1),&mut delivery);
    assert!(hidden["objects"][card.0.to_string()].is_null());
    let command=json!({"kind":"activate","source":card.0,"ability":5,"mana":{"C":1,"U":1,"B":1},"paying":[[attacker.0]],"targets":[]});
    let action=read_move(&command,&Pool::default()).unwrap();
    g.act(0,action).unwrap();
    let shown=state(&mut g,Some(1),&mut delivery);
    assert_eq!(shown["objects"][card.0.to_string()]["name"],"Kaito, Bane of Nightmares");
    let observations: Vec<_> = g.take_observations(1).iter().map(|o|disclosure(o)).collect();
    assert!(observations.iter().any(|o|o["kind"]=="revealed" && o["cards"][0]["object"]==card.0));
    assert!(g.counter(g.stack[0]));
    let hidden=state(&mut g,Some(1),&mut delivery);
    assert!(hidden["objects"][card.0.to_string()].is_null());
}

#[test]
fn r712_114_tamiyo_back_face_loyalty_and_unlimited_hand_size_are_visible_in_the_pipe() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let pair=faces.iter().filter(|f|f.name=="Tamiyo, Inquisitive Student // Tamiyo, Seasoned Scholar").collect::<Vec<_>>();
    let def=CardDef::from_faces(&pair).unwrap();check(&def).unwrap();
    let blank=CardDef::new("Blank","{0}","Artifact","",None).unwrap();
    let mut g=Game::new(vec![def,blank],2,20);for _ in 0..15 {g.put(0,1,Zone::Library);}
    let front=g.put(0,0,Zone::Battlefield);g.draw(0,3);
    g.act(0,Move::Pass).unwrap();
    while !g.stack.is_empty(){g.act(g.priority,Move::Pass).unwrap();}
    let back=*g.battlefield.last().unwrap();assert_ne!(front,back);
    let mut delivery=Delivery::default();let shown=state(&mut g,Some(1),&mut delivery);
    assert_eq!(shown["objects"][back.0.to_string()]["name"],"Tamiyo, Seasoned Scholar");
    assert_eq!(shown["objects"][back.0.to_string()]["counters"]["loyalty"],2);
    g.put_counters(back,"loyalty",5);
    g.act(0,read_move(&json!({"kind":"activate","source":back.0,"ability":2,"targets":[],"paying":[]}),&Pool::default()).unwrap()).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(1),&mut delivery);
    assert!(shown["players"][0]["maximum_hand_size"].is_null());
    assert_eq!(shown["players"][1]["maximum_hand_size"],7);
    assert!(shown["players"][0].get("hand").is_none());
    assert_eq!(shown["players"][0]["hand_size"],9);
}

#[test]
fn r702_138_escape_collective_cost_round_trips_through_the_pipe() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
    let def=CardDef::from_face(faces.iter().find(|f|f.name=="Nethergoyf").unwrap()).unwrap();
    let mut g=Game::new(vec![def,CardDef::new("Dual","{0}","Artifact Creature — Bear","",Some((2,2))).unwrap(),CardDef::new("Land","","Enchantment Land","",None).unwrap()],2,20);
    let card=g.put(0,0,Zone::Graveyard);let one=g.put(0,1,Zone::Graveyard);let two=g.put(0,2,Zone::Graveyard);g.players[0].pool.colors[2]=3;
    let insufficient=json!({"kind":"cast","card":card.0,"route":"escape","mana":{"B":3},"paying":[[one.0]],"targets":[]});
    let retained=format!("{g:?}");
    let reason=g.act(0,read_move(&insufficient,&g.players[0].pool).unwrap()).unwrap_err();
    assert!(reason.starts_with("that selection does not satisfy the cost: the chosen cards have fewer than 4 card types among them"), "{reason}");
    assert_eq!(format!("{g:?}"),retained);
    let selected=json!({"kind":"cast","card":card.0,"route":"escape","mana":{"B":3},"paying":[[one.0,two.0]],"targets":[]});
    let command=read_move(&selected,&g.players[0].pool).unwrap();g.act(0,command).unwrap();assert_eq!(g.exile.len(),2);assert_eq!(g.obj(card).zone,Zone::Gone);

}

#[cfg(test)]
#[test]
fn r702_56_and_707_10_copy_payment_and_target_choices_round_trip_through_json() {
    let d=CardDef::new("Consign to Memory","{U}","Instant","Replicate {1}\nCounter target triggered ability or colorless spell.",None).unwrap();check(&d).unwrap();
    let blank=CardDef::new("Blank","{0}","Artifact","",None).unwrap();
    let mut g=Game::new(vec![d,blank],2,20);let a=g.put(1,1,Zone::Stack);let b=g.put(1,1,Zone::Stack);let card=g.put(0,0,Zone::Hand);
    g.players[0].pool.colors[1]=1;g.players[0].pool.colorless=1;
    let command=json!({"kind":"cast","card":card.0,"replicate":1,"mana":{"C":1,"U":1},"targets":[[{"object":a.0}]]});
    g.act(0,read_move(&command,&g.players[0].pool).unwrap()).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let question=asked(&mut g);assert_eq!(question["kind"],"retarget");assert_eq!(question["slots"][0]["min"],1);
    let answer=json!({"kind":"choose","answer":{"targets":[[{"object":b.0}]]}});
    g.act(0,read_move(&answer,&Pool::default()).unwrap()).unwrap();
    let copy=*g.stack.last().unwrap();assert_eq!(g.obj(copy).targets,vec![vec![Target::Object(b)]]);
    assert_eq!(read_move(&json!({"kind":"pass","replicate":1}),&Pool::default()), Err("a pass move has no \"replicate\"; it says nothing more".into()));
    assert_eq!(read_move(&json!({"kind":"cast","card":card.0,"replicate":-1}),&Pool::default()), Err("replicate is a nonnegative whole number".into()));
}
#[test]
fn r702_29e_typecycling_uses_normal_activation_and_private_search_transport() {
    let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let lorien=CardDef::from_face(faces.iter().find(|f|f.name=="Lórien Revealed").unwrap()).unwrap();
    let mut g=Game::new(vec![lorien,CardDef::new("Island","","Basic Land — Island","",None).unwrap(),CardDef::new("Forest","","Basic Land — Forest","",None).unwrap()],2,20);
    let card=g.put(0,0,Zone::Hand);let found=g.put(0,1,Zone::Library);let forest=g.put(0,2,Zone::Library);g.players[0].pool.colorless=1;
    let ability = g.abilities(card).iter().position(|ability| matches!(&**ability, ir::Ability::Activated { .. })).unwrap();
    let chosen=json!({"kind":"activate","source":card.0,"ability":ability,"mana":{"C":1},"paying":[],"targets":[]});
    let command=read_move(&chosen,&g.players[0].pool).unwrap();g.act(0,command).unwrap();for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    let mine=state(&mut g,Some(0),&mut Delivery::default());let theirs=state(&mut g,Some(1),&mut Delivery::default());assert_eq!(mine["asked"]["kind"],"search");assert_eq!(mine["asked"]["among"],json!([found.0]));assert!(theirs["asked"]["among"].is_null());assert!(theirs["objects"][forest.0.to_string()].is_null());
    let command=read_move(&json!({"kind":"choose","answer":{"objects":[found.0]}}),&g.players[0].pool).unwrap();g.act(0,command).unwrap();assert_eq!(g.players[0].hand.len(),1);assert!(g.log.iter().any(|e|matches!(e,game::Event::Shuffled{player:0, ..})));assert!(g.log.iter().any(|e|matches!(e,game::Event::Revealed{cards,..} if cards==&vec![found])));
}

#[test]
fn r702_139_pipe_setup_special_action_and_selected_permission_preserve_authority_and_privacy(){
 let faces=corpus::load(&corpus::default_path()).unwrap();let catalog=Catalog::new(CardDef::catalog(&faces));let names=vec!["Memnite";30];let request=json!({"decks":[names,names,names],"companions":["Lurrus of the Dream-Den",null,null]});let mut g=start_game(&catalog,&request).unwrap();for p in 0..3{g.act(p,Move::Keep).unwrap();}while g.part!=ir::Part::PrecombatMain{g.act(g.priority,Move::Pass).unwrap();}g.players[0].pool.colorless=3;let mut delivery=Delivery::default();let shown=state(&mut g,Some(1),&mut delivery);assert_eq!(shown["players"][0]["companion"]["name"],"Lurrus of the Dream-Den");assert_eq!(shown["players"][0]["companion"]["taken"],false);assert!(shown["players"][0].get("hand").is_none());
 let said=json!({"kind":"companion","mana":{"C":3}});g.act(0,read_move(&said,&g.players[0].pool).unwrap()).unwrap();assert!(g.companion(0).unwrap().taken);let card=*g.players[0].hand.last().unwrap();g.players[0].pool.colors[0]=3;g.act(0,read_move(&json!({"kind":"cast","card":card.0,"mana":{"W":3},"targets":[]}),&g.players[0].pool).unwrap()).unwrap();for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}let in_hand=g.players[0].hand[0];g.move_object(in_hand,Zone::Graveyard);let card=g.players[0].graveyard[0];let said=json!({"kind":"cast","card":card.0,"targets":[],"route":{"source":g.battlefield[0].0,"ability":2}});g.act(0,read_move(&said,&g.players[0].pool).unwrap()).unwrap();for _ in 0..3{g.act(g.priority,Move::Pass).unwrap();}assert_eq!(g.battlefield.len(),2);
 let bad=json!({"decks":[vec!["Lurrus of the Dream-Den";30],names],"companions":["Lurrus of the Dream-Den",null]});assert_eq!(start_game(&catalog,&bad).unwrap_err(),"starting deck does not meet companion condition: Lurrus of the Dream-Den");let mut bad=request;bad["companions"]=json!([["Lurrus of the Dream-Den"],null,null]);assert_eq!(start_game(&catalog,&bad).unwrap_err(),"a companion is one card name or null");
}

#[cfg(test)]
#[test]
fn r601_2f_target_cost_partitions_round_trip_through_normal_cast_transport(){
 let faces=oracle::corpus::load(&oracle::corpus::default_path()).unwrap();let d=CardDef::from_face(faces.iter().find(|f|f.name=="Mystical Dispute").unwrap()).unwrap();check(&d).unwrap();
 let blue=CardDef::new("Blue answer","{U}","Instant","You gain 1 life.",None).unwrap();let red=CardDef::new("Red answer","{R}","Instant","You gain 1 life.",None).unwrap();let mut g=Game::new(vec![d,blue,red],2,20);let blue=g.put(1,1,Zone::Stack);let red=g.put(1,2,Zone::Stack);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=1;
 let mut command=json!({"kind":"cast","card":card.0,"mana":{"U":1},"targets":[[{"object":red.0}]]});let retained=format!("{g:?}");assert_eq!(g.act(0,read_move(&command,&g.players[0].pool).unwrap()),Err("the supplied mana does not pay that cost, all of it and no more (601.2h, 118.3a)".into()));assert_eq!(format!("{g:?}"),retained);command["targets"]=json!([[{"object":blue.0}]]);g.act(0,read_move(&command,&g.players[0].pool).unwrap()).unwrap();assert_eq!(g.players[0].pool.total(),0);
}

#[cfg(test)]
#[test]
fn r701_26_rewind_uses_the_normal_json_select_range_and_objects_answer() {
    let rewind=CardDef::new("Rewind","{2}{U}{U}","Instant","Counter target spell. Untap up to four lands.",None).unwrap();check(&rewind).unwrap();
    let land=CardDef::new("Island","","Basic Land — Island","",None).unwrap();
    let witness=CardDef::new("Witness","{0}","Instant","You gain 1 life.",None).unwrap();
    let mut g=Game::new(vec![rewind,land,witness],2,20);let lands:Vec<_>=(0..5).map(|p|{let o=g.put(p%2,1,Zone::Battlefield);g.set_tapped(o,true);o}).collect();
    let target=g.put(1,2,Zone::Stack);let card=g.put(0,0,Zone::Hand);g.players[0].pool.colors[1]=4;
    let command=json!({"kind":"cast","card":card.0,"mana":{"U":4},"targets":[[{"object":target.0}]]});g.act(0,read_move(&command,&g.players[0].pool).unwrap()).unwrap();
    for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    let question=asked(&mut g);assert_eq!(question["kind"],"select");assert_eq!(question["min"],0);assert_eq!(question["max"],4);assert_eq!(question["among"].as_array().unwrap().len(),5);
    for chosen in [vec![],vec![lands[1]],lands[..4].to_vec()]{let mut branch=g.clone();let answer=json!({"kind":"choose","answer":{"objects":ids(&chosen)}});branch.act(0,read_move(&answer,&Pool::default()).unwrap()).unwrap();assert_eq!(lands.iter().filter(|o|!branch.obj(**o).tapped).count(),chosen.len());}
}

#[cfg(test)]
#[test]
fn r903_9a_owner_command_zone_choice_uses_explicit_json_without_physical_identity() {
    let leader=CardDef::new("Leader","{0}","Legendary Creature — Human","",Some((2,2))).unwrap();
    let mut g=Game::new(vec![leader],3,40);
    let card=g.put(1,0,Zone::Command);g.designate_commander(card).unwrap();
    g.move_object(card,Zone::Graveyard);let card=g.players[1].graveyard[0];g.state_based_actions();
    let question=asked(&mut g);assert_eq!(question["kind"],"command_zone");assert_eq!(question["object"],card.0);
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    assert!(!shown.to_string().contains("card_id"));
    let command=read_move(&json!({"kind":"choose","answer":"yes"}),&Pool::default()).unwrap();
    let before=format!("{g:?}");
    assert_eq!(g.act(0,command.clone()),Err("that decision is another player's".into()));
    assert_eq!(format!("{g:?}"),before);
    g.act(1,command).unwrap();
    assert_eq!(g.command.len(),1);assert!(g.is_commander(g.command[0]));
}

#[cfg(test)]
#[test]
fn r903_5_managed_commander_setup_enforces_format_and_exposes_only_public_designation() {
    let faces=corpus::load(&corpus::default_path()).unwrap();
    let catalog=Catalog::new(CardDef::catalog(&faces));
    let base=json!({"format":"commander","decks":[vec!["Swamp";99],vec!["Swamp";99],vec!["Swamp";99]],
        "commanders":["Yargle, Glutton of Urborg","Yargle, Glutton of Urborg","Yargle, Glutton of Urborg"],"seed":7});
    let mut g=start_game(&catalog,&base).unwrap();
    let shown=state(&mut g,Some(1),&mut Delivery::default());
    assert_eq!(shown["command"].as_array().unwrap().len(),3);
    for p in 0..3 {
        assert_eq!(shown["players"][p]["life"],40);
        assert_eq!(shown["players"][p]["library"],92);
        assert_eq!(shown["players"][p]["commanders"][0]["name"],"Yargle, Glutton of Urborg");
        assert_eq!(shown["players"][p]["commanders"][0]["identity"],json!(["Black"]));
        assert_eq!(shown["players"][p]["commanders"][0]["casts"],json!([0,0,0]));
    }
    assert!(shown["players"][0].get("hand").is_none());
    assert!(!shown.to_string().contains("card_id"));
    for card in &g.command { assert_eq!(shown["objects"][card.0.to_string()]["commander"],true); }
    for (field,value,reason) in [
        ("life",json!(20),"Commander starting life is 40 (903.7)"),
        ("format",json!("constructed"),"designated commanders require format: commander"),
        ("commanders",json!([]),"Commander setup names one commander per player"),
        ("commanders",json!([null,null,null]),"one commander card name per player"),
        ("sideboards",json!([["Swamp"],[],[]]),"Commander games do not use sideboards (903.5e)"),
        ("companions",json!(["Lurrus of the Dream-Den",null,null]),"Commander companion setup is not implemented"),
    ] {
        let mut bad=base.clone();bad[field]=value;assert_eq!(start_game(&catalog,&bad).unwrap_err(),reason,"{field}");
    }
    let path=std::env::temp_dir().join(format!("commander-sideboard-{}.dck",std::process::id()));
    std::fs::write(&path,"99 Swamp\nSB: 1 Duress\n").unwrap();
    let mut bad=base;bad["decks"][0]=json!(path.to_str().unwrap());
    let rejected=start_game(&catalog,&bad).unwrap_err();std::fs::remove_file(path).unwrap();
    assert_eq!(rejected,"Commander games do not use sideboards (903.5e)");
}

#[cfg(test)]
#[test]
fn r712_8a_setup_never_accepts_a_detached_back_face_in_any_format() {
    let faces=corpus::load(&corpus::default_path()).unwrap();
    let catalog=Catalog::new(CardDef::catalog(&faces));
    for name in ["Avatar Kyoshi","Azamuki, Treachery Incarnate","Soporific Springs"] {
        let constructed=json!({"decks":[vec![name;60],vec!["Wastes";60]]});
        assert_eq!(start_game(&catalog,&constructed).unwrap_err(),format!("unknown card: {name}"));
        let commander=json!({"format":"commander","decks":[vec!["Wastes";99],vec!["Swamp";99]],
            "commanders":[name,"Yargle, Glutton of Urborg"]});
        assert_eq!(start_game(&catalog,&commander).unwrap_err(),format!("unknown card: {name}"));
    }
    let modal=json!({"decks":[vec!["Sink into Stupor";60],vec!["Island";60]]});
    assert!(start_game(&catalog,&modal).is_ok(),"whole modal cards preserve both supported faces");
}

#[cfg(test)]
#[test]
fn r305_7_snapshots_show_effective_land_subtypes_without_revealing_hidden_cards() {
    let faces=corpus::load(&corpus::default_path()).unwrap();let catalog=CardDef::catalog(&faces);
    let defs:Vec<_>=["Blood Moon","Temple Garden"].iter().map(|name|CardDef::from_faces(&catalog[name]).unwrap()).collect();
    let mut g=Game::new(defs,2,20);let land=g.put(0,1,Zone::Battlefield);let hidden=g.put(0,1,Zone::Hand);
    let moon=g.put(1,0,Zone::Battlefield);
    let seen=state(&mut g,Some(1),&mut Delivery::default());
    assert_eq!(seen["objects"][land.0.to_string()]["subtypes"],json!(["Mountain"]));
    assert!(seen["objects"].get(hidden.0.to_string()).is_none());
    g.move_object(moon,Zone::Graveyard);
    let seen=state(&mut g,Some(1),&mut Delivery::default());
    assert_eq!(seen["objects"][land.0.to_string()]["subtypes"],json!(["Forest","Plains"]));
}

/// Objects show their colors, so that devoid (702.114a) can be checked.
#[cfg(test)]
#[test]
fn r105_2_snapshots_show_colors_as_effects_leave_them() {
    let defs=vec![CardDef::new("Bear","{1}{G}","Creature — Bear","",Some((2,2))).unwrap(),
        CardDef::new("Scion","{1}{B}","Creature — Eldrazi","Devoid",Some((1,1))).unwrap(),
        CardDef::new("Charm","{W}{U}","Instant","Draw a card.",None).unwrap()];
    let mut g=Game::new(defs,2,20);
    let bear=g.put(0,0,Zone::Battlefield);let scion=g.put(0,1,Zone::Battlefield);let charm=g.put(1,2,Zone::Graveyard);
    let seen=state(&mut g,Some(1),&mut Delivery::default());
    assert_eq!(seen["objects"][bear.0.to_string()]["colors"],json!(["green"]));
    assert_eq!(seen["objects"][scion.0.to_string()]["colors"],json!([]),"devoid: colorless");
    assert_eq!(seen["objects"][charm.0.to_string()]["colors"],json!(["white","blue"]));
}

#[test]
fn r702_73_type_qualified_cast_permission_validates_announced_changeling() {
    let defs=vec![
        CardDef::new("Permission","{0}","Artifact","Once during each of your turns, you may cast a Goblin spell from your graveyard.",None).unwrap(),
        CardDef::new("Shape","{0}","Creature — Shapeshifter","Changeling",Some((3,2))).unwrap(),
        CardDef::new("Other","{0}","Creature — Bear","",Some((2,2))).unwrap()];
    let mut g=Game::new(defs,3,20);let source=g.put(0,0,Zone::Battlefield);
    let card=g.put(0,1,Zone::Graveyard);let wrong=g.put(0,2,Zone::Graveyard);
    let retained=format!("{g:?}");
    assert_eq!(g.act(0,read_move(&json!({"kind":"cast","card":wrong.0,"targets":[],"route":{"source":source.0,"ability":0}}),&Pool::default()).unwrap()), Err("that casting permission is unavailable (601.3)".into()));
    assert_eq!(format!("{g:?}"),retained);
    let command=json!({"kind":"cast","card":card.0,"targets":[],"route":{"source":source.0,"ability":0}});
    g.act(0,read_move(&command,&Pool::default()).unwrap()).unwrap();
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    let stack=*g.stack.last().unwrap();
    assert!(shown["objects"][stack.0.to_string()]["subtypes"].as_array().unwrap().contains(&json!("Goblin")));
}


#[cfg(test)]
#[test]
fn r616_1_player_counter_choice_and_public_maps_round_trip_through_json() {
    let defs=vec![
        CardDef::new("Plus","{0}","Enchantment","If one or more poison counters would be put on a player, that many plus one poison counters are put on that player instead.",None).unwrap(),
        CardDef::new("Times","{0}","Enchantment","If one or more poison counters would be put on a player, twice that many poison counters are put on that player instead.",None).unwrap(),
        CardDef::new("Poison","{0}","Sorcery","Target player gets a poison counter.",None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);
    let plus=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);
    let card=g.put(0,2,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![Target::Player(1)]]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let mut delivery=Delivery::default();
    let shown=state(&mut g,Some(1),&mut delivery);
    assert_eq!(shown["asked"]["player"],1);
    assert_eq!(shown["asked"]["on"],json!({"player":1}));
    assert_eq!(shown["players"][1]["counters"],json!({}));
    let answer=json!({"kind":"choose","answer":{"replacement":{"source":plus.0,"ability":0,"part":0}}});
    g.act(1,read_move(&answer,&Pool::default()).unwrap()).unwrap();
    for viewer in [Some(0),Some(1),None] {
        let shown=state(&mut g,viewer,&mut Delivery::default());
        assert_eq!(shown["players"][1]["counters"],json!({"poison":4}));
        assert_eq!(shown["players"][0]["counters"],json!({}));
    }
}

#[test]
fn r701_34_holder_offer_answer_and_delegation_round_trip() {
    let def=CardDef::new("Probe","{0}","Sorcery","Proliferate.",None).unwrap();
    let mut g=Game::new(vec![def],2,20);
    // The synthetic offer below tests serialization only; the live proliferate follows it.
    let among=vec![Target::Player(1),Target::Object(ObjId(7))];
    let offer=render_asked(&g,Some((0,Asked::Holders{verb:ir::Verb::Proliferate,among:among.clone()})));
    assert_eq!(offer["kind"],"holders");assert_eq!(offer["verb"],"Proliferate");
    assert_eq!(offer["among"],json!([{"player":1},{"object":7}]));
    for chosen in [vec![],among] {
        let mv=Move::Choose(Answer::Holders(chosen));
        assert_eq!(read_move(&answered(&mv),&Pool::default()).unwrap(),mv);
    }
    for (holders, reason) in [
        (json!(null), "holders: an array of offered players/permanents"),
        (json!(0), "holders: an array of offered players/permanents"),
        (json!([{"player":-1}]), "a target is {\"player\":n} or {\"object\":n}, not {\"player\":-1}"),
        (json!([{}]), "a target is {\"player\":n} or {\"object\":n}, not {}"),
    ] {
        assert_eq!(read_move(&json!({"kind":"choose","answer":{"holders":holders}}),&Pool::default()), Err(reason.into()));
    }
    g.put_counters(Target::Player(1),"energy",1);
    let card=g.put(0,0,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let asked=asked(&mut g);assert_eq!(asked["among"],json!([{"player":1}]));
    let mv=read_move(&json!({"kind":"choose","answer":{"holders":[{"player":1}]}}),&Pool::default()).unwrap();
    g.act(0,mv).unwrap();assert_eq!(g.counters(Target::Player(1))["energy"],2);
}

#[cfg(test)]
mod damage_choice_tests {
    use super::*;
    #[test]
    fn r616_1_damage_wire_requires_effect_and_hit_and_keeps_request_on_rejection() {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        let catalog = game::CardDef::catalog(&faces);
        let defs = vec![
            game::CardDef::from_faces(catalog.get("Furnace of Rath").unwrap()).unwrap(),
            game::CardDef::from_faces(catalog.get("Torbran, Thane of Red Fell").unwrap()).unwrap(),
            game::CardDef::new(
                "Twin",
                "{R}",
                "Instant",
                "This spell deals 3 damage to target player and 3 damage to each opponent.",
                None,
            )
            .unwrap(),
        ];
        for d in &defs {
            game::play::check(d).unwrap();
        }
        let mut g = Game::new(defs, 2, 20);
        g.players[0].pool.colors = [10; 5];
        let furnace = g.put(0, 0, Zone::Battlefield);
        let torbran = g.put(0, 1, Zone::Battlefield);
        let spell = g.put(0, 2, Zone::Hand);
        g.act(
            0,
            paid("R", Move::Cast {
                card: spell,
                targets: vec![vec![Target::Player(1)]],
            }),
        )
        .unwrap();
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["event"], "damage");
        assert_eq!(shown["asked"]["kind"], "replacement");
        assert_eq!(shown["asked"]["player"], 1);
        assert_eq!(shown["asked"]["among"].as_array().unwrap().len(), 2);
        assert_eq!(shown["asked"]["hits"].as_array().unwrap().len(), 1);
        assert_eq!(shown["asked"]["hits"][0]["amount"],6);
        let Some(game::state::Decision::Choose {
            choice: game::state::Choice::DamageReplacement { among, .. },
            ..
        }) = g.decision.clone()
        else {
            panic!("damage choice")
        };
        let good = json!({"kind":"choose","answer":{"replacement":{"source":furnace.0,"ability":0,"part":0,"hit":0}}});
        let displayed_offer=shown["asked"]["among"].as_array().unwrap().iter().find(|o|o["source"]==furnace.0&&o["hit"]==0).unwrap();
        let displayed=json!({"kind":"choose","answer":{"replacement":displayed_offer}});
        assert_eq!(read_damage_move(&displayed,&among).unwrap(),read_damage_move(&good,&among).unwrap());
        let mut missing = good.clone();
        missing["answer"]["replacement"]
            .as_object_mut()
            .unwrap()
            .remove("hit");
        assert_eq!(read_damage_move(&missing, &among),Err(Error::unreferable("that damage replacement/hit identity was not offered")));
        let mut stale = good.clone();
        stale["answer"]["replacement"]["hit"] = json!(999);
        assert_eq!(read_damage_move(&stale, &among),Err(Error::unreferable("that damage replacement/hit identity was not offered")));
        let retained=format!("{g:?}");
        assert_eq!(g.act(0, read_damage_move(&good, &among).unwrap()),Err("that decision is another player's".into()));
        assert_eq!(format!("{g:?}"),retained);
        assert_eq!(
            state(&mut g, Some(1), &mut Delivery::default())["asked"],
            shown["asked"]
        );
        // CR608.2f/614.5: the same simultaneous source-recipient amount is6,
        // so Furnace then Torbran yields14, while Torbran then Furnace yields16.
        for (mut branch, provider, life) in [(g.clone(),furnace,6),(g,torbran,4)] {
            let command=json!({"kind":"choose","answer":{"replacement":{"source":provider.0,"ability":0,"part":0,"hit":0}}});
            let mv=read_damage_move(&command,&among).unwrap();
            assert_eq!(answered(&mv),command);
            branch.act(1,mv).unwrap();
            assert!(branch.decision.is_none());
            assert_eq!(branch.players[1].life,life);
            let dealt:Vec<_>=branch.log.iter().filter_map(|e|match e {
                Event::DamageDealt{to:Target::Player(1),amount,..}=>Some(*amount),_=>None,
            }).collect();
            assert_eq!(dealt,vec![20-life as u32]);
            for viewer in 0..2 {
                assert_eq!(state(&mut branch,Some(viewer),&mut Delivery::default())["players"][1]["life"],life);
            }
        }
    }

}

#[cfg(test)]
mod prevention_choice_tests {
    use super::*;
    fn pinned(name: &str) -> game::CardDef {
        let faces = oracle::corpus::load(&oracle::corpus::default_path()).unwrap();
        game::CardDef::from_faces(game::CardDef::catalog(&faces).get(name).unwrap()).unwrap()
    }
    fn cast(g: &mut Game, card: usize, to: Target, mana: &str) {
        g.players[0].pool.colors = [10; 5];
        let spell = g.put(0, card, game::Zone::Hand);
        g.act(
            0,
            paid(mana, Move::Cast {
                card: spell,
                targets: vec![vec![to]],
            }),
        )
        .unwrap();
        for _ in 0..2 {
            g.act(g.priority, Move::Pass).unwrap();
        }
    }
    // 201.5b, 615.1a, 616.1: inherited prevention uses the borrower and retains the
    // exact donor acquisition on the wire, even after its donor leaves.
    #[test]
    fn r616_1_inherited_prevention_roundtrip_rejects_another_donor_acquisition() {
        let donor = game::CardDef::new("Shield Bear", "{0}", "Creature", "{0}: Prevent the next 1 damage that would be dealt to target creature this turn.", Some((4, 4))).unwrap();
        game::play::check(&donor).unwrap();
        let mut base = Game::new(vec![pinned("Marvin, Murderous Mimic"), donor, pinned("Furnace of Rath"), pinned("Char")], 2, 20);
        let marvin = base.put(0, 0, game::Zone::Battlefield);
        let first = base.put(0, 1, game::Zone::Battlefield);
        base.put(0, 1, game::Zone::Battlefield);
        base.put(0, 2, game::Zone::Battlefield);
        let prompt = Simulation::new(base.clone()).prompt(0).unwrap();
        let shown = render_prompt(&base, &prompt, 1, &mut Delivery::default(), &[]);
        let shown_marvin = shown["objects"].as_object().unwrap().values().find(|o| o["name"] == "Marvin, Murderous Mimic").unwrap();
        for ability in shown_marvin["abilities"].as_array().unwrap() {
            assert_eq!(ability["granted_by"], labelled(|| oid(marvin)));
        }
        let mut games = [base.clone(), base];
        let mut offers = Vec::new();
        for (index, g) in games.iter_mut().enumerate() {
            g.act(0, Move::Activate { source: marvin, ability: index + 1, targets: vec![vec![Target::Object(marvin)]], paying: vec![] }).unwrap();
            if index == 0 { g.move_object(first, game::Zone::Graveyard); }
            let prompt = Simulation::new(g.clone()).prompt(0).unwrap();
            let shown = render_prompt(g, &prompt, 1, &mut Delivery::default(), &[]);
            let occurrence = shown["objects"].as_object().unwrap().values().find_map(|o| o.get("occurrence")).unwrap();
            assert_eq!(occurrence["granted"], json!(true));
            assert_eq!(occurrence["source"], labelled(|| oid(marvin)));
            assert_eq!(occurrence["source_name"], json!("Marvin, Murderous Mimic"));
            for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
            let source = g.put(0, 3, game::Zone::Stack);
            g.damage(source, Target::Object(marvin), 1);
            let Some(game::state::Decision::Choose { choice: game::state::Choice::DamageOrder { among, .. }, .. }) = g.decision.clone() else { panic!("ordering prevention against doubling") };
            let shields:Vec<_>=among.iter().filter(|o|damage_option(o)["kind"]=="shield").collect();
            assert_eq!(shields.len(),1,"bind the one shield created by this branch's authored donor activation");
            offers.push((damage_option(shields[0]), among));
        }
        assert_ne!(offers[0].0["ability"], offers[1].0["ability"]);
        let command = |offer: &Value| json!({"kind":"choose", "answer":{"damage_order":offer}});
        assert_eq!(read_damage_order(&command(&offers[1].0), &offers[0].1),Err(Error::unreferable("that damage/prevention identity was not offered")));
        let mv = read_damage_order(&command(&offers[0].0), &offers[0].1).unwrap();
        let before = state(&mut games[0], Some(0), &mut Delivery::default())["asked"].clone();
        let mut clone = games[0].clone();
        for g in [&mut games[0], &mut clone] {
            assert_eq!(state(g, Some(0), &mut Delivery::default())["asked"], before);
            g.act(0, mv.clone()).unwrap();
            assert_eq!(g.obj(marvin).damage, 0); // prevention first consumes the only damage
        }
    }
    /// Rules: 608.2f, 615.7. Char's two clauses damage the same player from one source.
    #[test]
    fn r615_7_char_to_self_consumes_one_shield_without_an_allocation_choice() {
        let mut g=Game::new(vec![pinned("Mending Hands"),pinned("Char")],2,20);
        cast(&mut g,0,Target::Player(0),"W");
        cast(&mut g,1,Target::Player(0),"RRR");
        assert!(g.decision.is_none(),"one source needs no allocation choice");
        assert_eq!(g.players[0].life,18,"prevent4 of simultaneous6, not4 independently per fragment");
        cast(&mut g,1,Target::Player(0),"RRR");
        assert!(g.decision.is_none());
        assert_eq!(g.players[0].life,12,"the shield was fully consumed by the first Char");
    }

    #[test]
    fn r615_7_full_mending_takedown_allocation_roundtrips_and_invalid_answers_keep_the_question() {
        let mut g=Game::new(vec![pinned("Mending Hands"),pinned("Graceful Takedown"),
            CardDef::new("Four","{0}","Creature","",Some((4,5))).unwrap(),
            CardDef::new("Two","{0}","Creature","",Some((2,3))).unwrap(),
            CardDef::new("Wall","{0}","Creature","",Some((0,20))).unwrap(),
            CardDef::new("Aura","{0}","Enchantment — Aura","Enchant creature",None).unwrap()],2,20);
        let four=g.put(0,2,Zone::Battlefield);let two=g.put(0,3,Zone::Battlefield);
        let wall=g.put(1,4,Zone::Battlefield);let aura=g.put(0,5,Zone::Battlefield);g.attach(aura,four.into());
        cast(&mut g,0,wall.into(),"W");
        let takedown=g.put(0,1,Zone::Hand);
        g.act(0,paid("GG",Move::Cast{card:takedown,targets:vec![vec![four.into()],vec![two.into()],vec![wall.into()]]})).unwrap();
        for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["player"],1);
        assert_eq!(shown["asked"]["kind"], json!("prevention"));
        assert_eq!(shown["asked"]["total"], json!(4));
        let Some(game::state::Decision::Choose {
            choice: game::state::Choice::Prevention { hits, .. },
            ..
        }) = g.decision.clone()
        else {
            panic!("allocation")
        };
        assert_eq!(hits.iter().map(|h|h.source).collect::<Vec<_>>(),[four,two]);
        assert_eq!(hits.iter().map(|h| h.amount).collect::<Vec<_>>(), [4, 2]);
        let retained=format!("{g:?}");
        for (values,reason) in [
            (json!(null),"prevention: an array of hit/amount pairs"),
            (json!([[0,-1],[1,5]]),"prevention: nonnegative integer amount"),
            (json!([[0,4294967296u64],[1,0]]),"prevention: amount overflow"),
            (json!([[0,1.5],[1,2.5]]),"prevention: nonnegative integer amount"),
        ] {
            assert_eq!(read_move(&json!({"kind":"choose","answer":{"prevention":values}}),&Pool::default()),Err(reason.into()));
        }
        for values in [
            json!([[0, 2], [0, 2]]),
            json!([[0, 3], [1, 0]]),
            json!([[0, 0], [1, 4]]),
            json!([[99, 2], [1, 2]]),
        ] {
            let mv = read_move(
                &json!({"kind":"choose","answer":{"prevention":values}}),
                &Pool::default(),
            )
            .unwrap();
            assert_eq!(g.act(1,mv),Err("allocate the required shield total over each offered hit within its capacity (615.7)".into()));
            assert_eq!(format!("{g:?}"),retained);
            assert_eq!(
                state(&mut g, Some(1), &mut Delivery::default())["asked"],
                shown["asked"]
            );
        }
        let command = json!({"kind":"choose","answer":{"prevention":[[0,2],[1,2]]}});
        let mv = read_move(&command, &Pool::default()).unwrap();
        assert_eq!(answered(&mv), command);
        let mut copy = g.clone();
        for game in [&mut g, &mut copy] {
            game.act(1, mv.clone()).unwrap();
            assert!(game.stack.is_empty());
            assert!(game.decision.is_none());
            for viewer in 0..2 {
                assert_eq!(
                    state(game, Some(viewer), &mut Delivery::default())["objects"][wall.0.to_string()]["damage"],
                    json!(2)
                );
            }
            game.damage(four, wall.into(), 1);
            assert_eq!(game.obj(wall).damage, 3, "the full shield was consumed by allocation");
            assert!(game.decision.is_none());
        }
    }
    #[test]
    fn r616_1_group_and_per_hit_offers_have_complete_distinct_wire_identities() {
        let mut g = Game::new(
            vec![
                pinned("Mending Hands"),
                pinned("Furnace of Rath"),
                pinned("Char"),
            ],
            2,
            20,
        );
        g.put(0, 1, game::Zone::Battlefield);
        cast(&mut g, 0, Target::Player(1), "W");
        let source = g.put(0, 2, game::Zone::Stack);
        g.damage(source, Target::Player(1), 3);
        let shown = state(&mut g, Some(1), &mut Delivery::default());
        assert_eq!(shown["asked"]["event"], json!("damage_prevention"));
        let Some(game::state::Decision::Choose {
            choice: game::state::Choice::DamageOrder { among, .. },
            ..
        }) = g.decision.clone()
        else {
            panic!("order")
        };
        for offer in shown["asked"]["among"].as_array().unwrap() {
            assert!(
                read_damage_order(
                    &json!({"kind":"choose","answer":{"damage_order":offer}}),
                    &among
                )
                .is_ok()
            );
        }
        assert_eq!(read_damage_order(
            &json!({"kind":"choose","answer":{"damage_order":{"kind":"protection","hit":0}}}), &among),
            Err(Error::unreferable("that damage/prevention identity was not offered")));
        let mut wrong = damage_option(&among[0]);
        wrong["hit"] = json!(99);
        assert_eq!(read_damage_order(
            &json!({"kind":"choose","answer":{"damage_order":wrong}}), &among),
            Err(Error::unreferable("that damage/prevention identity was not offered")));
    }
}

#[cfg(test)]
mod buyback_wire_tests {
    use super::*;
    #[test]
    fn bool_and_spell_scoped_destination_are_strict() {
        for paid in [true,false] {
            assert!(matches!(read_move(&json!({"kind":"cast","card":1,"targets":[],"buyback":paid}),&Pool::default()).unwrap(),Move::WithBuyback(p,_) if p==paid));
            assert!(matches!(read_move(&json!({"kind":"cast","card":1,"targets":[],"bargain":paid}),&Pool::default()).unwrap(),Move::WithOptional(p,_) if p==paid));
        }
        for invalid in [json!(null),json!(1),json!("true"),json!([])] {
            assert_eq!(read_move(&json!({"kind":"cast","card":1,"targets":[],"buyback":invalid}),&Pool::default()), Err("buyback must be a boolean".into()));
        }
        assert_eq!(read_move(&json!({"kind":"pass","buyback":false}),&Pool::default()), Err("a pass move has no \"buyback\"; it says nothing more".into()));
        for effect in ["buyback","rebound","cast_rider","omen","flashback","graveyard:0"] {
            assert!(matches!(read_move(&json!({"kind":"choose","answer":{"spell_destination":{"on":42,"effect":effect}}}),&Pool::default()).unwrap(),Move::Choose(Answer::SpellDestination{on:ObjId(42),..})));
        }
        assert_eq!(read_move(&json!({"kind":"choose","answer":{"spell_destination":{"effect":"buyback"}}}),&Pool::default()), Err("an object id, not null".into()));
        // Omen/Flashback now participate in completion replacement choices (720.3d,
        // 702.34a, 616.1); reject unknown or malformed effects, not those valid names.
        for effect in ["unknown", "graveyard:", "graveyard:-1", "graveyard:nope"] {
            assert_eq!(read_move(&json!({"kind":"choose","answer":{"spell_destination":{"on":42,"effect":effect}}}),&Pool::default()), Err("spell_destination: unknown effect".into()));
        }
    }
}

#[cfg(test)]
mod entwine_wire_tests {
    use super::*;
    #[test]
    fn explicit_entwine_is_a_cast_only_boolean_and_absence_is_unchanged() {
        for paid in [false,true]{assert!(matches!(read_move(&json!({"kind":"cast","card":1,"targets":[],"entwine":paid}),&Pool::default()).unwrap(),Move::WithEntwine(p,_) if p==paid));}
        for invalid in [json!(null),json!(1),json!("true")]{assert_eq!(read_move(&json!({"kind":"cast","card":1,"targets":[],"entwine":invalid}),&Pool::default()), Err("entwine must be a boolean".into()));}
        assert_eq!(read_move(&json!({"kind":"pass","entwine":false}),&Pool::default()), Err("a pass move has no \"entwine\"; it says nothing more".into()));
        assert!(matches!(read_move(&json!({"kind":"cast","card":1,"targets":[]}),&Pool::default()).unwrap(),Move::Cast{..}));
    }
}


#[test]
fn r608_2d_token_instruction_options_are_named_and_round_trip() {
    let d=CardDef::new("Instruction","{0}","Sorcery","Create a Food token or a Treasure token.",None).unwrap();
    let mut g=Game::new(vec![d],2,20);let card=g.put(0,0,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![]}).unwrap();g.act(0,Move::Pass).unwrap();g.act(1,Move::Pass).unwrap();
    let question=asked(&mut g);
    assert_eq!(question["kind"],"alternative");assert_eq!(question["optional"],false);
    assert_eq!(question["among"],json!([
        {"index":0,"action":"Create","tokens":[{"name":"Food","count":1}]},
        {"index":1,"action":"Create","tokens":[{"name":"Treasure","count":1}]}]));
    let command=json!({"kind":"choose","answer":{"alternative":1}});
    let mv=read_move(&command,&Pool::default()).unwrap();assert_eq!(answered(&mv),command);
    g.act(0,mv).unwrap();assert_eq!(g.battlefield.len(),1);
}

#[cfg(test)]
mod optional_cost_wire_tests {
    use super::*;
    /// A spell's optional additional cost is listed and answered under the name of what it is
    /// (601.2b, 118.8b): Intrepid Rabbit's listings say `"offspring"` paid and not, and the
    /// listing that pays it, sent back, makes the 1/1 token copy (702.175a).
    #[test]
    fn r702_175a_offspring_is_answered_by_name() {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let rabbit = CardDef::from_faces(catalog.get("Intrepid Rabbit").unwrap()).unwrap();
        let plains = CardDef::new("Plains", "", "Basic Land — Plains", "({T}: Add {W}.)", None).unwrap();
        let mut g = Game::new(vec![rabbit, plains], 2, 20);
        for p in 0..2 { for _ in 0..4 { g.put(p, 1, Zone::Library); } }
        let card = g.put(0, 0, Zone::Hand);
        let lands: Vec<_> = (0..4).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        // the key names what the card's cost is: another cost's name is refused, not paid
        let offspring = card;
        assert!(optional_named(&json!({"kind":"cast","card":offspring.0,"offspring":true}), &g).is_ok());
        for name in ["teamwork", "bargain", "additional", "casualty"] {
            let refused = optional_named(&json!({"kind":"cast","card":offspring.0,name:true}), &g).unwrap_err();
            assert_eq!(refused,format!("that spell's optional additional cost is offspring, not {name} (601.2b)"));
        }
        assert_eq!(optional_named(&json!({"kind":"cast","card":card.0 + 1000,"teamwork":true}), &g), Err(Error::unreferable("an object handle is stale or unknown")));
        // The parser preserves both wrappers; optional-name validation rejects the combination.
        let twice = json!({"kind":"cast","card":card.0,"targets":[],"offspring":true,"bargain":true});
        assert!(matches!(read_move(&twice, &Pool::default()), Ok(Move::WithOptional(true, inner)) if matches!(*inner, Move::WithOptional(true, _))));
        assert_eq!(optional_named(&twice,&g),Err("a cast names one optional additional cost".into()));
        let plan: Vec<_> = lands.iter().map(|o|json!({"source":o.0,"ability":0,"paying":[],"adding":[]})).collect();
        let chosen = json!({"kind":"cast","card":card.0,"targets":[],"offspring":true,"mana":{"W":4},"plan":plan});
        g.act(0, read_move(&chosen, &Pool::default()).unwrap()).unwrap();
        for _ in 0..20 {
            match &g.decision {
                Some(Decision::Targets { player, .. }) => {
                    let p = *player;
                    // Bind the unique nontoken incarnation of the authored Rabbit; both ETBs target it.
                    let originals:Vec<_>=g.battlefield.iter().copied().filter(|o|g.obj(*o).card==Some(0)&&!g.obj(*o).token()).collect();
                    assert_eq!(originals.len(),1);
                    g.act(p, Move::ChooseTargets(vec![vec![Target::Object(originals[0])]])).unwrap();
                }
                Some(Decision::Order { player, .. }) => { let p = *player; g.act(p, Move::Order(vec![0, 1])).unwrap(); }
                None if g.stack.is_empty() => break,
                _ => g.act(g.priority, Move::Pass).unwrap(),
            }
        }
        let token = *g.battlefield.iter().find(|o| g.obj(**o).token()).expect("the offspring token");
        assert_eq!((g.power(token), g.toughness(token)), (Some(1), Some(1)));
        assert_eq!(g.battlefield.len(),6); // Four authored Plains plus Rabbit and offspring.
        assert!(g.stack.is_empty());
        let original=*g.battlefield.iter().find(|o|g.obj(**o).card==Some(0)&&!g.obj(**o).token()).unwrap();
        assert_eq!((g.power(original),g.toughness(original)),(Some(5),Some(4)), "both authored ETB targets are the original Rabbit");

    }
}

#[cfg(test)]
mod cost_choice_wire_tests {
    use super::*;
    /// A cost that is a choice among alternatives is listed once for each payable one, with
    /// `"chose"` sent back and `"alternatives"` saying them (601.2b): Bitter Triumph's "discard
    /// a card or pay 3 life", with no card to discard, offers only the life.
    #[test]
    fn r601_2b_bitter_triumph_lists_and_answers_its_cost_choice() {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let triumph = CardDef::from_faces(catalog.get("Bitter Triumph").unwrap()).unwrap();
        let swamp = CardDef::new("Swamp", "", "Basic Land — Swamp", "({T}: Add {B}.)", None).unwrap();
        let bear = CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
        let mut g = Game::new(vec![triumph, swamp, bear], 2, 20);
        for p in 0..2 { for _ in 0..4 { g.put(p, 1, Zone::Library); } }
        let card = g.put(0, 0, Zone::Hand);
        let lands: Vec<_> = (0..2).map(|_| g.put(0, 1, Zone::Battlefield)).collect();
        let victim = g.put(1, 2, Zone::Battlefield);
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let plan: Vec<_> = lands.iter().map(|o|json!({"source":o.0,"ability":0,"paying":[],"adding":[]})).collect();
        let chosen = json!({"kind":"cast","card":card.0,"targets":[[{"object":victim.0}]],"mana":{"B":2},"plan":plan,"chose":[1]});
        g.act(0, read_move(&chosen, &Pool::default()).unwrap()).unwrap();
        assert_eq!(g.players[0].life, 17);
        assert_eq!(read_move(&json!({"kind":"pass","chose":[0]}), &Pool::default()), Err("a pass move has no \"chose\"; it says nothing more".into()));
        assert_eq!(read_move(&json!({"kind":"cast","card":card.0,"targets":[],"chose":["life"]}), &Pool::default()), Err("chose is a list of alternatives, one number for each choice of the cost".into()));
    }
}

#[cfg(test)]
mod gift_wire_tests {
    use super::*;
    /// A gift is promised to an opponent by its player label, as player/1 names players
    /// everywhere else, and the promise is public: every seat's view of the spell on the stack
    /// says it (702.174a). A seat number, as recordings answered, is still read.
    #[test]
    fn r702_174a_a_gift_is_promised_by_player_label_and_every_seat_sees_it() {
        let gift = CardDef::new("Offering", "{0}", "Instant", "Gift a card\nYou gain 2 life.", None).unwrap();
        check(&gift).unwrap();
        let island = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
        let new = || {
            let mut game = Game::new(vec![gift.clone(), island.clone()], 2, 20);
            for p in 0..2 { for _ in 0..3 { game.put(p, 1, Zone::Library); } }
            game.put(0, 0, Zone::Hand);
            while game.part != ir::Part::PrecombatMain { game.act(game.priority, Move::Pass).unwrap(); }
            Some(Simulation::new(game))
        };
        let faces = Catalog::new(BTreeMap::new());
        // each game its own session and delivery
        let fresh = || (new(), 1, Delivery::default());
        let send = |(simulation, session, logged): &mut (Option<Simulation>, u64, Delivery), request: Value| dispatch(&faces, simulation, session, logged, &request);
        for answered_by in ["label", "number"] {
            let mut simulation = fresh();
            let prompt = send(&mut simulation, json!({"cmd":"prompt","seat":"p0"})).unwrap();
            let id = prompt["request_id"].clone();
            let card = prompt["players"][0]["hand"][0].clone();
            let mut promised = json!({"kind":"cast","card":card,"targets":[],"gift":"p1"});
            if answered_by == "number" { promised["gift"] = json!(1); }
            let answer = |id: &Value, answer: Value| json!({"cmd":"answer","seat":"p0","session_id":1,"request_id":id,"answer":answer});
            // not a player label: refused, saying the form expected
            let mut wrong = promised.clone();
            wrong["gift"] = json!("them");
            let refused = send(&mut simulation, answer(&id, wrong)).unwrap_err();
            assert!(refused.contains("player label") && refused.contains("\"p1\""), "{refused}");
            // an object's label, the card's own: said as such, not as the object it is read as
            let mut object = promised.clone();
            object["gift"] = promised["card"].clone();
            let refused = send(&mut simulation, answer(&id, object)).unwrap_err();
            assert!(refused.ends_with("not an object label"), "{refused}");
            send(&mut simulation, answer(&id, promised)).unwrap();
            // each seat, the promised opponent too, sees the gift promised to p1 on the stack
            for seat in ["p0", "p1"] {
                let view = send(&mut simulation, json!({"cmd":"prompt","seat":seat})).unwrap();
                let spell = view["stack"][0].as_str().expect("the spell on the stack").to_owned();
                assert_eq!(view["objects"][&spell]["gift"], json!("p1"), "{answered_by}: {seat} sees {}", view["objects"][&spell]);
            }
        }
        // an unpromised spell says no gift
        let mut simulation = fresh();
        let prompt = send(&mut simulation, json!({"cmd":"prompt","seat":"p0"})).unwrap();
        let id = prompt["request_id"].clone();
        let card = prompt["players"][0]["hand"][0].clone();
        let unpromised = json!({"kind":"cast","card":card,"targets":[]});
        send(&mut simulation, json!({"cmd":"answer","seat":"p0","session_id":1,"request_id":id,"answer":unpromised})).unwrap();
        let view = send(&mut simulation, json!({"cmd":"prompt","seat":"p1"})).unwrap();
        let spell = view["stack"][0].as_str().unwrap().to_owned();
        assert!(view["objects"][&spell].get("gift").is_none());
    }
}

#[cfg(test)]
mod public_cost_choice_wire_tests {
    use super::*;
    fn cards(names: &[&str], more: Vec<CardDef>) -> Vec<CardDef> {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        names.iter().map(|n| CardDef::from_faces(catalog.get(*n).unwrap()).unwrap()).chain(more).collect()
    }
    fn to_main(mut g: Game) -> Game {
        for p in 0..2 { for _ in 0..4 { g.put(p, 1, Zone::Library); } }
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        g
    }
    /// The alternative announced is public (601.2b): beholding a Kithkin for Kinsbaile
    /// Aspirant's "behold a Kithkin or pay {2}", each seat's view of the spell says the
    /// alternative as the listing said it, and the permanent beheld is an event told to each
    /// seat (701.4a), so the opponent can tell it from {2} paid.
    #[test]
    fn r601_2b_every_seat_sees_the_alternative_announced_and_the_permanent_beheld() {
        let defs = cards(&["Kinsbaile Aspirant"], vec![
            CardDef::new("Wastes", "", "Basic Land", "{T}: Add {C}.", None).unwrap(),
            CardDef::new("Kithkin", "{W}", "Creature — Kithkin", "", Some((1, 1))).unwrap(),
        ]);
        let mut g = Game::new(defs, 2, 20);
        g.put(0, 0, Zone::Hand);
        g.put(0, 2, Zone::Battlefield);
        let mut g = to_main(g);
        g.players[0].pool.colors[0] = 1;
        let faces = Catalog::new(BTreeMap::new());
        let mut session = (Some(Simulation::new(g)), 1, Delivery::default());
        let mut send = |request: Value| { let (simulation, session, logged) = &mut session; dispatch(&faces, simulation, session, logged, &request) };
        // the opponent has looked at the game before the spell is cast
        send(json!({"cmd":"prompt","seat":"p1"})).unwrap();
        let prompt = send(json!({"cmd":"prompt","seat":"p0"})).unwrap();
        let id = prompt["request_id"].clone();
        let card = prompt["players"][0]["hand"][0].clone();
        let kithkin = prompt["battlefield"].as_array().unwrap().iter()
            .find(|id| prompt["objects"][id.as_str().unwrap()]["name"] == "Kithkin").unwrap().clone();
        let answer = json!({"kind":"cast","card":card,"targets":[],"mana":{"W":1},"chose":[0],"paying":[[kithkin]]});
        // the answering seat is told in the prompt its answer returns, the other as it looks
        let after = send(json!({"cmd":"answer","seat":"p0","session_id":1,"request_id":id,"answer":answer})).unwrap();
        for seat in ["p0", "p1"] {
            let view = if seat == "p0" { after.clone() } else { send(json!({"cmd":"prompt","seat":seat})).unwrap() };
            let spell = view["stack"][0].as_str().expect("the spell on the stack").to_owned();
            assert_eq!(view["objects"][&spell]["announced"], json!(["behold a Kithkin"]), "{seat}: {}", view["objects"][&spell]);
            let events = view["events"].as_array().unwrap();
            assert!(events.iter().any(|e| e["event"] == "beheld" && e["player"] == "p0" && e["permanents"] == json!([kithkin])), "{seat}: {events:?}");
        }
    }
    /// A mana ability whose cost is a choice among alternatives is a step of a listed plan
    /// that says which it pays (601.2g, 605.3a): Thornvault Forager forages for the Bear with
    /// its Food, the step `"chose":[1]`; the listing answered as it is casts it.
    #[test]
    fn r605_3a_a_plan_step_says_the_alternative_of_its_cost() {
        let defs = cards(&["Thornvault Forager"], vec![
            CardDef::new("Wastes", "", "Basic Land", "{T}: Add {C}.", None).unwrap(),
            CardDef::new("Bear", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap(),
            CardDef::new("Food", "", "Artifact — Food", "{2}, {T}, Sacrifice this artifact: You gain 3 life.", None).unwrap(),
        ]);
        let mut g = Game::new(defs, 2, 20);
        let bear = g.put(0, 2, Zone::Hand);
        let forager = g.put(0, 0, Zone::Battlefield);
        let food = g.put(0, 3, Zone::Battlefield);
        let mut g = to_main(g);
        let chosen=json!({"kind":"cast","card":bear.0,"targets":[],"mana":{"G":2},"plan":[
            {"source":forager.0,"ability":1,"paying":[[food.0]],"adding":[["G","G"]],"chose":[1]}]});
        let unannounced=g.clone();
        g.act(0, read_move(&chosen, &Pool::default()).unwrap()).unwrap();
        assert!(!g.battlefield.contains(&food) && g.stack.len() == 1);
        // a step that says no alternative, or a word for one, is refused
        let mut unsaid = chosen.clone();
        for step in unsaid["plan"].as_array_mut().unwrap() { step.as_object_mut().unwrap().remove("chose"); }
        let mut unannounced=unannounced;
        let before=format!("{unannounced:?}");
        assert_eq!(unannounced.act(0,read_move(&unsaid,&Pool::default()).unwrap()),Err("that cost has 1 choice(s) among alternatives: say which alternative of each is paid or applied (601.2b, 118.7e)".into()));
        assert_eq!(format!("{unannounced:?}"),before);
        let mut worded = chosen.clone();
        for step in worded["plan"].as_array_mut().unwrap() { if step.get("chose").is_some() { step["chose"] = json!(["Food"]); } }
        assert_eq!(read_move(&worded, &Pool::default()),Err("step cost choice must be an integer".into()));
    }
}

#[cfg(test)]
#[test]
fn r616_1_mixed_counter_wire_shows_whole_inventory_actor_and_modified_proposal() {
    // One instruction, one holder, three replacement identities. After Plus, two
    // doublers remain, so the next actual question must show the modified inventory.
    let plus="If you would put one or more counters on a permanent you control, put that many plus one of each of those kinds of counters on that permanent instead.";
    let times="If one or more counters would be put on a creature you control, twice that many of each of those kinds of counters are put on it instead.";
    let defs=vec![CardDef::new("Plus","{0}","Enchantment",plus,None).unwrap(),
        CardDef::new("Times A","{0}","Enchantment",times,None).unwrap(),
        CardDef::new("Times B","{0}","Enchantment",times,None).unwrap(),
        CardDef::new("Bear","{0}","Creature","",Some((2,2))).unwrap(),
        CardDef::new("Inventory","{0}","Instant","Put a charge counter and two stun counters on target creature.",None).unwrap()];
    let mut g=Game::new(defs,2,20);let plus=g.put(0,0,Zone::Battlefield);let times=g.put(0,1,Zone::Battlefield);g.put(0,2,Zone::Battlefield);
    let bear=g.put(0,3,Zone::Battlefield);let card=g.put(0,4,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![bear.into()]]}).unwrap();for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let mut delivery=Delivery::default();let shown=state(&mut g,Some(0),&mut delivery);
    assert_eq!(shown["asked"]["on"],json!({"object":bear.0}));assert_eq!(shown["asked"]["by"],0);
    assert_eq!(shown["asked"]["counter"],Value::Null);
    assert_eq!(shown["asked"]["counters"],json!([{"counter":"charge","count":1},{"counter":"stun","count":2}]));
    let answer=|source:ObjId|json!({"kind":"choose","answer":{"replacement":{"source":source.0,"ability":0,"part":0}}});
    g.act(0,read_move(&answer(plus),&Pool::default()).unwrap()).unwrap();
    let shown=state(&mut g,Some(0),&mut delivery);
    assert_eq!(shown["asked"]["counters"],json!([{"counter":"charge","count":2},{"counter":"stun","count":3}]));
    assert_eq!(shown["asked"]["among"].as_array().unwrap().len(),2);
    assert!(g.obj(bear).counters.is_empty());
    g.act(0,read_move(&answer(times),&Pool::default()).unwrap()).unwrap();
    assert_eq!(g.obj(bear).counters.get("charge"),Some(&8));assert_eq!(g.obj(bear).counters.get("stun"),Some(&12));
}

#[cfg(test)]
#[test]
fn r616_1_entry_question_shows_existing_whole_counter_inventory() {
    let defs=vec![
        CardDef::new("Plus","{0}","Enchantment","If you would put one or more counters on a permanent you control, put that many plus one of each of those kinds of counters on that permanent instead.",None).unwrap(),
        CardDef::new("Times","{0}","Enchantment","If one or more counters would be put on a creature you control, twice that many of each of those kinds of counters are put on it instead.",None).unwrap(),
        CardDef::new("Entrant","{0}","Creature","This creature enters with a charge counter on it.\nThis creature enters with a stun counter on it.",Some((2,2))).unwrap()];
    let mut g=Game::new(defs,2,20);let plus=g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);
    let card=g.put(0,2,Zone::Hand);g.act(0,Move::Cast{card,targets:vec![]}).unwrap();for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    for row in [0,1] {
        let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&g.decision else{panic!("entry question")};
        let id=among.iter().find(|id|g.obj(id.source).chars.name=="Entrant" && id.ability().is_some_and(|key|key.index==row)).unwrap().clone();
        g.act(0,Move::Choose(game::state::Answer::Replacement(id))).unwrap();
    }
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    assert_eq!(shown["asked"]["event"],"entry");assert_eq!(shown["asked"]["by"],0);
    assert_eq!(shown["asked"]["counter"],Value::Null);assert_eq!(shown["asked"]["count"],Value::Null);
    assert_eq!(shown["asked"]["counters"],json!([{"counter":"charge","count":1},{"counter":"stun","count":1}]));
    let answer=json!({"kind":"choose","answer":{"replacement":{"source":plus.0,"ability":0,"part":0}}});
    g.act(0,read_move(&answer,&Pool::default()).unwrap()).unwrap();
    let entrant=*g.battlefield.iter().find(|o|g.obj(**o).chars.name=="Entrant").unwrap();
    assert_eq!(g.obj(entrant).counters.get("charge"),Some(&4));assert_eq!(g.obj(entrant).counters.get("stun"),Some(&4));
}

#[cfg(test)]
#[test]
fn r107_1b_large_mixed_inventory_question_has_exact_kinds_without_overflowing_total() {
    let defs=vec![
        CardDef::new("Plus","{0}","Enchantment","If you would put one or more counters on a permanent, put that many plus one of each of those kinds of counters on it instead.",None).unwrap(),
        CardDef::new("Double","{0}","Enchantment","If you would put one or more counters on a permanent, put twice that many of each of those kinds of counters on it instead.",None).unwrap(),
        CardDef::new("Holder","{0}","Artifact","",None).unwrap(),
        CardDef::new("Inventory","{0}","Instant","Put 4294967295 charge counters and 4294967295 stun counters on target artifact.",None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);
    let holder=g.put(0,2,Zone::Battlefield);let card=g.put(0,3,Zone::Hand);
    g.act(0,Move::Cast{card,targets:vec![vec![holder.into()]]}).unwrap();for _ in 0..2{g.act(g.priority,Move::Pass).unwrap();}
    let shown=state(&mut g,Some(0),&mut Delivery::default());
    assert_eq!(shown["asked"]["counter"],Value::Null);assert_eq!(shown["asked"]["count"],Value::Null);
    assert_eq!(shown["asked"]["counters"],json!([{"counter":"charge","count":4294967295u32},{"counter":"stun","count":4294967295u32}]));
}

#[cfg(test)]
#[test]
fn r701_7_creation_replacement_question_displays_exact_original_token_inventory() {
    let defs=vec![
        CardDef::new("Double","{0}","Enchantment","If one or more tokens would be created under your control, twice that many of those tokens are created instead.",None).unwrap(),
        CardDef::new("Triple Creature","{0}","Enchantment","If one or more creature tokens would be created under your control, three times that many of those tokens are created instead.",None).unwrap(),
        CardDef::new("Maker","{0}","Instant","Create a 1/1 white Soldier creature token and a Treasure token.",None).unwrap(),
    ];
    let mut g=Game::new(defs,2,20);g.put(0,0,Zone::Battlefield);g.put(0,1,Zone::Battlefield);
    let card=g.put(0,2,Zone::Hand);g.act(0,Move::Cast {card,targets:vec![]}).unwrap();
    for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
    let q=asked(&mut g);
    assert_eq!(q["kind"],"replacement");assert_eq!(q["event"],"create");
    assert_eq!(q["count"],Value::Null);assert_eq!(q["by"],0);
    // CR 111.3, 701.7b: Maker defines a white 1/1 Soldier, with no abilities,
    // mana cost or back face. The wire retains those copiable values before entry.
    assert_eq!(q["tokens"][0],json!({"name":"Soldier Token","count":1,"types":["Creature"],"colors":["white"],"power":1,"toughness":1,"tapped":false,"choose_defender":false,"attacking":null,"attached_to":null,
        "specification":{"front":{"name":"Soldier Token","types":["Creature"],"supertypes":[],"subtypes":["Soldier"],"colors":["White"],"power_toughness":[1,1],"mana_value":0,"mana_cost":null,"keywords":[],"predefined":null,"rows":[]},"back":null,"transformed":false}}));
    assert_eq!(q["tokens"][1]["name"],"Treasure Token");assert_eq!(q["tokens"][1]["count"],1);
    assert_eq!(q["tokens"][1]["types"],json!(["Artifact"]));
    assert_eq!(q["among"].as_array().unwrap().len(),2);
    assert_eq!(g.battlefield.len(),2);
}

#[cfg(test)]
mod finality_wire_tests {
    use super::*;
    #[test]
    fn r122_1h_rule_owned_departure_identity_round_trips_and_rejects_spoofed_ability() {
        let bear=CardDef::new("Bear","{0}","Creature — Bear","If this creature would die, put it on the bottom of its owner's library instead.",Some((2,2))).unwrap();
        let kill=CardDef::new("Kill","{0}","Instant","Destroy target creature.",None).unwrap();
        let mut g=Game::new(vec![bear,kill],2,20);let source=g.put(0,0,Zone::Battlefield);g.put_counters(source,"finality",2);
        let spell=g.put(0,1,Zone::Hand);g.act(0,Move::Cast{card:spell,targets:vec![vec![game::Target::Object(source)]]}).unwrap();
        for _ in 0..2 {g.act(g.priority,Move::Pass).unwrap();}
        let Some(game::state::Decision::Choose{choice:game::state::Choice::Replacement{among,..},..})=&g.decision else {panic!("departure choice")};
        let offered=among.clone();
        let rule=game::operation::ReplacementId {source,effect:game::operation::ReplacementEffect::Finality};
        assert!(offered.contains(&rule));
        let shown=asked(&mut g);assert_eq!(shown["among"].as_array().unwrap().len(),2);
        assert_eq!(replacement(&rule),json!({"source":source.0,"rule":"finality"}));
        assert!(shown["among"].as_array().unwrap().iter().any(|e|e["rule"]=="finality" && e.get("ability").is_none()));
        let said=json!({"kind":"choose","answer":{"replacement":replacement(&rule)}});
        let mv=read_move_offered(&said,&Pool::default(),&offered).unwrap();assert_eq!(mv,Move::Choose(Answer::Replacement(rule.clone())));
        let mut wrong=said.clone();wrong["answer"]["replacement"]["source"]=json!(spell.0);
        assert_eq!(read_move_offered(&wrong,&Pool::default(),&offered),Err(Error::unreferable("that replacement identity was not offered")));
        wrong=said.clone();wrong["answer"]["replacement"]["ability"]=json!(0);
        assert_eq!(read_move_offered(&wrong,&Pool::default(),&offered),Err(Error::unreferable("that replacement identity was not offered")));
        assert_eq!(read_move_offered(&said,&Pool::default(),&[]),Err(Error::unreferable("that replacement identity was not offered")));
        let mut copy=g.clone();copy.act(0,mv.clone()).unwrap();g.act(0,mv).unwrap();
        assert_eq!(g.exile.len(),1);assert_eq!(copy.exile.len(),1);assert!(g.players[0].library.is_empty());
    }
}

/// The wire forms a replay of a game recorded elsewhere drives: recorded initial orders
/// (103.3), concession (104.3a), whether the first turn has begun (103.8), what a value or a
/// select question chooses, whose a skip or a spell destination effect is (616.1), and
/// questions with one possible answer asked all the same (508.4).
#[cfg(test)]
mod replay_wire_tests {
    use super::*;

    const DECK: [&str; 10] = ["Plains", "Island", "Swamp", "Mountain", "Forest", "Wastes", "Grizzly Bears", "Llanowar Elves", "Lightning Bolt", "Giant Growth"];

    fn with_corpus(test: impl FnOnce(&Catalog)) {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = Catalog::new(CardDef::catalog(&faces));
        test(&catalog);
    }

    #[test]
    fn r103_3_start_with_recorded_orders_puts_each_library_in_that_order() {
        with_corpus(|catalog| {
            let (mut game, mut session, mut logged) = (None, 0, Delivery::default());
            let orders = json!([[9, 8, 7, 6, 5, 4, 3, 2, 1, 0], [3, 1, 4, 0, 5, 9, 2, 6, 8, 7]]);
            let start = json!({"cmd":"start", "decks":[DECK, DECK], "first":0, "orders":orders});
            let state = dispatch(catalog, &mut game, &mut session, &mut logged, &start).unwrap();
            // 103.8: the mulligans are asked; the first turn has not begun
            assert_eq!(state["asked"]["kind"], "mulligan");
            assert_eq!(state["begun"], false);
            for player in 0..2 {
                let order: Vec<usize> = orders[player].as_array().unwrap().iter().map(|i| i.as_u64().unwrap() as usize).collect();
                let library = dispatch(catalog, &mut game, &mut session, &mut logged,
                    &json!({"cmd":"library_order", "session_id":state["session_id"], "player":player})).unwrap();
                let g = game.as_ref().unwrap().game().unwrap();
                let name = |id: &Value| g.obj(ObjId(id.as_u64().unwrap())).chars.name.clone();
                // the top seven were drawn (103.5); the rest are in the recorded order
                let names: Vec<String> = library["order"].as_array().unwrap().iter().map(name).collect();
                let expected: Vec<String> = order[7..].iter().map(|i| DECK[*i].to_string()).collect();
                assert_eq!(names, expected, "player {player}");
                let mut hand: Vec<String> = state["players"][player]["hand"].as_array().unwrap().iter().map(name).collect();
                let mut top: Vec<String> = order[..7].iter().map(|i| DECK[*i].to_string()).collect();
                hand.sort(); top.sort();
                assert_eq!(hand, top, "player {player}");
            }
            let mut send = |input: Value| dispatch(catalog, &mut game, &mut session, &mut logged, &input);
            let mut seeded = start.clone();
            seeded["seed"] = json!(7);
            assert_eq!(send(seeded).unwrap_err(), "orders and seed are exclusive: with orders no random number is drawn");
            let mut short = start.clone();
            short["orders"] = json!([[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]]);
            assert_eq!(send(short).unwrap_err(), "orders gives one library order per player");
            let mut repeated = start;
            repeated["orders"][1] = json!([0, 0, 1, 2, 3, 4, 5, 6, 7, 8]);
            assert!(send(repeated).unwrap_err().contains("not a permutation"));
        });
    }

    #[test]
    fn r103_4_a_starting_life_total_the_engine_cannot_hold_is_refused_not_truncated() {
        with_corpus(|catalog| {
            let (mut game, mut session, mut logged) = (None, 0, Delivery::default());
            let mut send = |input: Value| dispatch(catalog, &mut game, &mut session, &mut logged, &input);
            let refused = format!("starting life is a whole number in the engine range {} to {}", i32::MIN, i32::MAX);
            // 2^32 + 20 was read as 20 when truncated to 32 bits (107.1b)
            for life in [json!(4_294_967_316_i64), json!(i64::from(i32::MAX) + 1), json!(i64::from(i32::MIN) - 1), json!(20.5), json!("20")] {
                let start = json!({"cmd":"start", "decks":[DECK, DECK], "first":0, "seed":3, "life":life});
                assert_eq!(send(start).unwrap_err(), refused, "{life}");
            }
            // each player begins with the starting life total (103.4), to the bound
            for life in [i32::MAX, 20, 30] {
                let state = send(json!({"cmd":"start", "decks":[DECK, DECK], "first":0, "seed":3, "life":life})).unwrap();
                assert_eq!((&state["players"][0]["life"], &state["players"][1]["life"]), (&json!(life), &json!(life)));
            }
            let state = send(json!({"cmd":"start", "decks":[DECK, DECK], "first":0, "seed":3})).unwrap();
            assert_eq!(state["players"][0]["life"], 20);
        });
    }

    #[test]
    fn r103_8_begun_says_when_the_first_turn_has_begun() {
        with_corpus(|catalog| {
            let (mut game, mut session, mut logged) = (None, 0, Delivery::default());
            let mut send = |input: Value| dispatch(catalog, &mut game, &mut session, &mut logged, &input);
            let mut state = send(json!({"cmd":"start", "decks":[DECK, DECK], "first":0, "seed":3})).unwrap();
            for _ in 0..2 {
                assert_eq!((&state["begun"], &state["turn"]), (&json!(false), &json!(1)));
                state = send(json!({"cmd":"act", "session_id":state["session_id"], "request_id":state["request_id"],
                    "player":state["asked"]["player"], "move":{"kind":"keep"}})).unwrap();
            }
            assert_eq!(state["begun"], true);
        });
    }

    #[test]
    fn r104_3a_concede_over_the_wire_ends_a_two_player_game() {
        let (mut game, mut session, mut logged, faces) = (None, 0, Delivery::default(), Catalog::new(BTreeMap::new()));
        let mut send = |input: Value| dispatch(&faces, &mut game, &mut session, &mut logged, &input);
        let state = send(json!({"cmd":"new", "players":[{}, {}]})).unwrap();
        assert_eq!(state["asked"]["player"], 0);
        let concede = json!({"cmd":"act", "session_id":state["session_id"], "request_id":state["request_id"],
            "player":1, "move":{"kind":"concede"}});
        let mut extra = concede.clone();
        extra["move"]["card"] = json!(1);
        assert_eq!(send(extra), Err("a concede move has no \"card\"; it says nothing more".into()));
        let retained=send(json!({"cmd":"state"})).unwrap();
        assert_eq!(retained["finished"],false);
        assert_eq!(retained["request_id"],state["request_id"]);
        // player 1 concedes while player 0 is asked (104.3a)
        let over = send(concede.clone()).unwrap();
        assert_eq!((&over["finished"], &over["winner"], &over["asked"]), (&json!(true), &json!(0), &Value::Null));
        let events: Vec<&Value> = over["events"].as_array().unwrap().iter().filter(|e| e["event"] == "conceded" || e["event"] == "lost").collect();
        assert_eq!(events, [&json!({"event":"conceded","player":1}), &json!({"event":"lost","player":1})]);
        assert!(over["request_id"].is_null(), "a finished game publishes no pending request");
        // Reusing the consumed request is stale; replacing it with null would only test JSON shape.
        let stale=send(concede).unwrap_err();
        assert!(stale.starts_with(&format!("stale request {}; expected ",state["request_id"])), "{stale}");
        let retained=send(json!({"cmd":"state"})).unwrap();
        assert_eq!((retained["finished"].clone(),retained["winner"].clone(),retained["asked"].clone()), (json!(true),json!(0),Value::Null));
        assert_eq!(answered(&Move::Concede), json!({"kind":"concede"}));
    }

    #[test]
    fn r105_1_a_value_question_says_what_is_chosen() {
        for (text, of) in [("color", "color"), ("creature type", "creature_type"), ("card name", "card_name")] {
            let d = CardDef::new("Witness", "", "Land", &format!("As this land enters, choose a {text}."), None).unwrap();
            check(&d).unwrap();
            let mut g = Game::new(vec![d], 2, 20);
            let card = g.put(0, 0, Zone::Hand);
            g.act(0, Move::PlayLand(card)).unwrap();
            let request = asked(&mut g);
            assert_eq!((&request["kind"], &request["of"]), (&json!("value"), &json!(of)));
        }
        assert_eq!(value_kind(game::binding::Kind::BasicLandType), "basic_land_type");
        assert_eq!(value_kind(game::binding::Kind::CardType), "card_type");
    }

    #[test]
    fn r700_3_the_separation_into_piles_is_labelled() {
        let faces = corpus::load(&corpus::default_path()).unwrap();
        let catalog = CardDef::catalog(&faces);
        let jace = CardDef::from_faces(catalog.get("Jace, Architect of Thought").unwrap()).unwrap();
        let land = CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap();
        let mut g = Game::new(vec![jace, land], 2, 20);
        for _ in 0..3 { g.put(0, 1, Zone::Library); }
        let source = g.put(0, 0, Zone::Battlefield);
        g.put_counters(source, "loyalty", 4);
        g.act(0, Move::Activate { source, ability: 1, targets: vec![], paying: vec![] }).unwrap();
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        // the opponent separates the three cards (700.3)
        let request = asked(&mut g);
        assert_eq!((&request["kind"], &request["player"], &request["separate"]), (&json!("select"), &json!(1), &json!(true)));
        let first = request["among"][0].clone();
        g.act(1, read_move(&json!({"kind":"choose", "answer":{"objects":[first]}}), &Pool::default()).unwrap()).unwrap();
        let request = asked(&mut g);
        assert_eq!(request["kind"], "pile");
        assert!(request.get("separate").is_none());
    }

    #[test]
    fn r616_1_an_offered_skip_effect_names_the_object_whose_effect_it_is() {
        let defs = vec![
            CardDef::new("Island", "", "Basic Land — Island", "", None).unwrap(),
            CardDef::new("Lasting Peace", "{0}", "Instant", "Target player skips their next combat phase.", None).unwrap(),
            CardDef::new("Brief Peace", "{0}", "Instant", "Target player skips their next combat phase this turn.", None).unwrap(),
        ];
        for d in &defs { check(d).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        for p in 0..2 { for _ in 0..5 { g.put(p, 0, Zone::Library); } }
        g.begin();
        while g.part != ir::Part::PrecombatMain { g.act(g.priority, Move::Pass).unwrap(); }
        let mut spells = Vec::new();
        for def in [1, 2] {
            let card = g.put(0, def, Zone::Hand);
            g.act(0, Move::Cast { card, targets: vec![vec![Target::Player(0)]] }).unwrap();
            spells.push(g.stack[0]);
            while !g.stack.is_empty() { g.act(g.priority, Move::Pass).unwrap(); }
        }
        g.act(0, Move::Pass).unwrap();
        g.act(1, Move::Pass).unwrap();
        let request = asked(&mut g);
        assert_eq!(request["kind"], "skip");
        // each skip effect is the effect of the spell that created it (614.10, 616.1)
        let sources: Vec<&Value> = request["among"].as_array().unwrap().iter().map(|s| &s["source"]).collect();
        assert_eq!(sources, [&json!(spells[0].0), &json!(spells[1].0)]);
        assert!(request["among"].as_array().unwrap().iter().all(|s| s.get("ability").is_some()));
    }

    #[test]
    fn r616_1_spell_destination_effects_are_named_with_their_source() {
        let d = CardDef::new("Recurring Gift", "{0}", "Instant", "Buyback {0}\nRebound\nYou gain 1 life.", None).unwrap();
        check(&d).unwrap();
        let mut g = Game::new(vec![d], 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        g.act(0, Move::WithBuyback(true, Box::new(Move::Cast { card, targets: vec![] }))).unwrap();
        let spell = g.stack[0];
        for _ in 0..2 { g.act(g.priority, Move::Pass).unwrap(); }
        let request = asked(&mut g);
        assert_eq!((&request["event"], &request["on"]), (&json!("spell_destination"), &json!({"object":spell.0})));
        // buyback (702.27a) and rebound (702.88a) are the spell's own keyword abilities: its
        // first and second paragraphs
        assert_eq!(request["among"], json!([
            {"effect":"buyback", "source":spell.0, "ability":0},
            {"effect":"rebound", "source":spell.0, "ability":1},
        ]));
        // the answer is named as before
        g.act(0, read_move(&json!({"kind":"choose", "answer":{"spell_destination":{"on":spell.0, "effect":"buyback"}}}), &Pool::default()).unwrap()).unwrap();
        assert!(g.players[0].hand.iter().any(|c| g.obj(*c).chars.name == "Recurring Gift"));
    }

    /// Hero of Bladehold attacks alone into a player without planeswalkers: each Soldier it
    /// creates attacking has one possible defender (508.4). Answers each question until the
    /// declare blockers step; returns the defender questions asked.
    fn hero_attacks(catalog: &Catalog, ask_forced: Option<bool>) -> Vec<Value> {
        let (mut game, mut session, mut logged) = (None, 0, Delivery::default());
        let mut send = |input: Value| dispatch(catalog, &mut game, &mut session, &mut logged, &input);
        let library = vec!["Island"; 5];
        let mut setup = json!({"cmd":"new", "players":[{"battlefield":["Hero of Bladehold"], "library":library}, {"library":library}]});
        if let Some(ask) = ask_forced { setup["ask_forced"] = json!(ask); }
        let mut state = send(setup).unwrap();
        let hero = state["battlefield"][0].clone();
        let mut asked_defenders = Vec::new();
        while state["part"] != "DeclareBlockers" {
            let asked = state["asked"].clone();
            let mv = match asked["kind"].as_str().unwrap() {
                "attackers" => json!({"kind":"attack", "attacks":[[hero, {"player":1}]]}),
                "order" => json!({"kind":"order", "order":(0..asked["abilities"].as_array().unwrap().len()).collect::<Vec<_>>()}),
                "defender" => { asked_defenders.push(asked.clone()); json!({"kind":"choose", "answer":{"defender":asked["among"][0]}}) }
                "priority" => json!({"kind":"pass"}),
                other => panic!("unexpected question {other}: {asked}"),
            };
            state = send(json!({"cmd":"act", "session_id":state["session_id"], "request_id":state["request_id"],
                "player":asked["player"], "move":mv})).unwrap();
        }
        asked_defenders
    }

    #[test]
    fn r508_4_ask_forced_asks_the_defender_of_a_creature_entering_attacking_with_one_choice() {
        with_corpus(|catalog| {
            assert!(hero_attacks(catalog, None).is_empty(), "by default one possible defender is chosen unasked");
            assert!(hero_attacks(catalog, Some(false)).is_empty());
            let asked = hero_attacks(catalog, Some(true));
            assert_eq!(asked.len(), 2, "one question for each Soldier: {asked:?}");
            for (n, question) in asked.iter().enumerate() {
                assert_eq!(question["among"], json!([{"player":1}]));
                assert_eq!((&question["nth"], &question["of"]), (&json!(n + 1), &json!(2)));
            }
        });
    }
}

#[cfg(test)]
mod payment_quote_tests {
    use super::*;
    fn setup(cost:&str,producer:&str) -> (Game,ObjId,ObjId) {
        let defs=vec![CardDef::new("Test Spell",cost,"Creature — Bear","",Some((2,2))).unwrap(),
            CardDef::new("Test Land","","Land",producer,None).unwrap()];
        let mut g=Game::new(defs,2,20);
        let spell=g.put(0,0,Zone::Hand);
        let land=g.put(0,1,Zone::Battlefield);
        (g,spell,land)
    }
    #[test]
    fn r601_2h_r404_3_suspended_payment_quote_binds_actual_order_and_finishes_once() {
        let defs=vec![CardDef::new("Fireblast","{4}{R}{R}","Instant","You may sacrifice two Mountains rather than pay this spell's mana cost.\nFireblast deals 4 damage to any target.",None).unwrap(),
            CardDef::new("Mountain","","Basic Land — Mountain","",None).unwrap()];
        let mut g=Game::new(defs,2,20);
        let card=g.put(0,0,Zone::Hand);let one=g.put(0,1,Zone::Battlefield);let two=g.put(0,1,Zone::Battlefield);
        let mut host=Some(Simulation::new(g));
        let (mut session,mut delivery,faces)=(1,Delivery::default(),Catalog::new(BTreeMap::new()));
        let state=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"state"})).unwrap();
        let command=json!({"kind":"cast","card":card.0,"cost":{"alternative":0},"targets":[[{"player":1}]],"paying":[[one.0,two.0]],"mana":{}});
        let quote_request=json!({"cmd":"quote_payment","session_id":1,"request_id":state["request_id"],"player":0,"move":command});
        let before=format!("{:?}",host.as_ref().unwrap().game().unwrap());
        let quote=dispatch(&faces,&mut host,&mut session,&mut delivery,&quote_request).unwrap();
        assert_eq!(quote["complete"],false);assert!(quote["error"].is_null());
        assert_eq!(quote["suspended"]["asked"],json!({"kind":"order_cards","player":0,"to":{"zone":"graveyard"},"cards":[one.0,two.0]}));
        assert_eq!(quote["suspended"]["announced"]["cast"]["completed"],false);
        assert_eq!(quote["suspended"]["source"],quote["payments"][0]["source"]);
        assert_eq!(quote,dispatch(&faces,&mut host,&mut session,&mut delivery,&quote_request).unwrap());
        assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),before);
        for field in ["source","asked","complete","receipts"] {
            let mut forged=quote.clone();
            match field {"source"=>forged["suspended"]["source"]=json!(999),"asked"=>forged["suspended"]["asked"]["cards"]=json!([two.0,one.0]),"complete"=>forged["complete"]=json!(true),_=>forged["payments"]=json!([])};
            assert_eq!(dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,"request_id":state["request_id"],"player":0,"move":command,"payment_expect":forged})).unwrap_err(),"payment quote changed or is incomplete; action was not applied");
            assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),before);
        }
        let waiting=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,"request_id":state["request_id"],"player":0,"move":command,"payment_expect":quote})).unwrap();
        let at_order=format!("{:?}",host.as_ref().unwrap().game().unwrap());
        for (player,cards,reason) in [(1,vec![two.0,one.0],"request belongs to player 0, not 1"),(0,vec![one.0,one.0],"order every offered card exactly once (401.4, 404.3)")] {
            assert_eq!(dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,"request_id":waiting["request_id"],"player":player,"move":{"kind":"choose","answer":{"objects":cards}}})).unwrap_err(),reason);
            assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),at_order);
        }
        dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,"request_id":waiting["request_id"],"player":0,"move":{"kind":"choose","answer":{"objects":[two.0,one.0]}}})).unwrap();
        let live=host.as_ref().unwrap().game().unwrap();
        assert_eq!(live.players[0].graveyard.iter().map(|o|live.physical_card_identity(*o)).collect::<Vec<_>>(),[live.physical_card_identity(one),live.physical_card_identity(two)]);
        assert_eq!(live.stack.len(),1);
        assert_eq!(live.log.iter().filter(|e|matches!(e,game::Event::Cast{..})).count(),1);
        assert_eq!(live.log.iter().filter(|e|matches!(e,game::Event::Sacrificed{..})).count(),2);
    }
    #[test]
    fn r601_2gh_quote_reports_locked_cost_production_and_leftovers_without_mutation() {
        let (mut g,spell,land)=setup("{1}{G}","{T}: Add {G}{G}.");
        g.players[0].pool.colors[3]=1;
        let before=format!("{g:?}");
        let command=json!({"kind":"cast","card":spell.0,"plan":[{"source":land.0,"ability":0}],"mana":{"R":1,"G":1}});
        let q=payment_quote(&g,0,&command).unwrap();
        assert_eq!(q["complete"],true,"{q}");
        let payment=q["payments"].as_array().unwrap().last().unwrap();
        assert_eq!(payment["requirement"]["symbols"],json!(["1","G"]));
        assert_eq!((payment["pool"]["R"].as_u64(),payment["pool"]["G"].as_u64()),(Some(1),Some(2)));
        assert_eq!((payment["left"]["R"].as_u64(),payment["left"]["G"].as_u64()),(Some(0),Some(1)));
        assert!(q["events"].as_array().unwrap().iter().any(|e|e["event"]=="mana_added" && e["mana"]==json!(["G","G"])));
        assert_eq!(format!("{g:?}"),before);
        assert!(g.payment_audit().is_empty());
        assert_eq!(payment_quote(&g,0,&command).unwrap(),q);
    }
    #[test]
    fn r107_4f_phyrexian_life_choice_preserves_printed_allocation_indices() {
        let (mut g,spell,_)=setup("{G/P}{R/G}","{T}: Add {G}.");
        g.players[0].pool.colors[4]=1;
        let command=json!({"kind":"cast","card":spell.0,"life":[true],"mana":{"G":1}});
        let q=payment_quote(&g,0,&command).unwrap();
        assert_eq!(q["complete"],true,"{q}");
        assert_eq!(q["symbol_indices"],json!([1]));
        assert_eq!(q["payments"][0]["requirement"]["symbols"],json!(["R/G"]));
        assert_eq!(g.players[0].life,20);
    }
    #[test]
    fn r106_6_post_plan_restricted_mana_is_selected_from_actual_production() {
        let (g,spell,land)=setup("{G}","{T}: Add {G}. Spend this mana only to cast a creature spell.");
        let command=json!({"kind":"cast","card":spell.0,"plan":[{"source":land.0,"ability":0}],"mana":{"restricted_after":[0]}});
        let q=payment_quote(&g,0,&command).unwrap();
        assert_eq!(q["complete"],true,"{q}");
        assert_eq!(q["payments"][1]["selected"]["restricted"][0]["mana"],"G");
        assert_eq!(q["payments"][1]["left"]["restricted"],json!([]));
    }
    #[test]
    fn r601_2h_changed_quote_is_rejected_before_live_payment() {
        let (mut g,spell,land)=setup("{G}","{T}: Add {G}.");
        g.players[0].pool.colors[4]=1;
        let mut host=Some(Simulation::new(g));
        let (mut session,mut delivery,faces)=(1,Delivery::default(),Catalog::new(BTreeMap::new()));
        let state=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"state"})).unwrap();
        let command=json!({"kind":"cast","card":spell.0,"mana":{"G":1}});
        let request=json!({"cmd":"quote_payment","session_id":1,"request_id":state["request_id"],"player":0,"move":command});
        let mut quote=dispatch(&faces,&mut host,&mut session,&mut delivery,&request).unwrap();
        assert_eq!(quote["complete"],true);
        quote["pools"][0]["G"]=json!(99);
        let error=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,
            "request_id":state["request_id"],"player":0,"move":command,"payment_expect":quote})).unwrap_err();
        assert!(error.contains("not applied"));
        let live=host.as_ref().unwrap().game().unwrap();
        assert!(live.players[0].hand.contains(&spell));
        assert_eq!(live.players[0].pool.colors[4],1);
        assert!(!live.obj(land).tapped);
        assert!(live.payment_audit().is_empty());
    }
    #[test]
    fn r118_3_unpaid_non_order_quote_is_not_an_authenticated_suspension() {
        let (g,spell,_)=setup("{G}","{T}: Add {G}.");
        let before=format!("{g:?}");
        let command=json!({"kind":"cast","card":spell.0,"mana":{}});
        let quote=payment_quote(&g,0,&command).unwrap();
        assert_eq!(quote["complete"],false);
        assert!(quote.get("suspended").is_none());
        assert!(quote["payments"][0]["left"].is_null());
        assert_eq!(format!("{g:?}"),before);
        let mut host=Some(Simulation::new(g));
        let (mut session,mut delivery,faces)=(1,Delivery::default(),Catalog::new(BTreeMap::new()));
        let state=dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"state"})).unwrap();
        assert_eq!(dispatch(&faces,&mut host,&mut session,&mut delivery,&json!({"cmd":"act","session_id":1,"request_id":state["request_id"],"player":0,"move":command,"payment_expect":quote})).unwrap_err(),"payment quote changed or is incomplete; action was not applied");
        assert_eq!(format!("{:?}",host.as_ref().unwrap().game().unwrap()),before);
    }
}

#[cfg(test)]
mod explicit_pool_wire_tests {
    use super::*;

    #[test]
    fn r601_2h_pool_counts_are_explicit_and_malformed_selections_never_become_zero() {
        // Wire counts are exact nonnegative u32 values, never coerced to zero.
        // The same field contract applies to the outer and each nested payment.
        for (mana, reason) in [
            (json!(null), "mana selection is an object"),
            (json!([]), "mana selection is an object"),
            (json!(1), "mana selection is an object"),
            (json!({"R":-1}), "mana R is a nonnegative u32 count"),
            (json!({"R":1.5}), "mana R is a nonnegative u32 count"),
            (json!({"R":"1"}), "mana R is a nonnegative u32 count"),
            (json!({"R":4294967296u64}), "mana R is a nonnegative u32 count"),
            (json!({"restricted":null}), "restricted mana selection is a list of indices"),
        ] {
            let cast = json!({"kind":"cast", "card":0, "mana":mana});
            assert_eq!(read_move(&cast, &Pool::default()).unwrap_err(), reason, "{cast}");
            let cast = json!({"kind":"cast", "card":0, "plan":[{"source":1,"ability":0,"mana":mana}]});
            assert_eq!(read_move(&cast, &Pool::default()).unwrap_err(), reason, "{cast}");
        }
        assert!(read_move(&json!({"kind":"cast","card":0}), &Pool::default()).is_ok());
        assert!(read_move(&json!({"kind":"cast","card":0,"mana":{}}), &Pool::default()).is_ok());
    }

    #[test]
    fn r605_3_nested_cost_and_outer_cost_each_require_their_own_supplied_mana() {
        let defs = vec![CardDef::new("Gift", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
            CardDef::new("Filter", "", "Land", "{1}, {T}: Add {G}{G}.", None).unwrap()];
        let mut g = Game::new(defs, 2, 20);
        let card = g.put(0, 0, Zone::Hand); let source = g.put(0, 1, Zone::Battlefield);
        g.players[0].pool.colors[3] = 1;
        let before = format!("{g:?}");
        let mut command = json!({"kind":"cast","card":card.0,"plan":[{"source":source.0,"ability":0}],"mana":{"G":1}});
        let q = payment_quote(&g, 0, &command).unwrap();
        assert_eq!(q["complete"], false);
        assert_eq!(q["payments"].as_array().unwrap().len(), 1);
        assert_eq!(q["payments"][0]["requirement"]["symbols"], json!(["1"]));
        assert!(q["payments"][0]["left"].is_null());
        assert!(q["symbol_indices"].is_null(), "child cost cannot inherit outer printed indices");
        assert_eq!(format!("{g:?}"), before);
        command["plan"][0]["mana"] = json!({"R":1});
        command.as_object_mut().unwrap().remove("mana");
        let q = payment_quote(&g, 0, &command).unwrap();
        assert_eq!(q["complete"], false);
        assert_eq!(q["payments"].as_array().unwrap().len(), 2);
        assert!(q["payments"][1]["left"].is_null());
        command["mana"] = json!({"G":1});
        let q = payment_quote(&g, 0, &command).unwrap();
        assert_eq!(q["complete"], true, "{q}");
        assert_eq!((q["payments"][1]["left"]["R"].as_u64(), q["payments"][1]["left"]["G"].as_u64()), (Some(0), Some(1)));
        assert_eq!(format!("{g:?}"), before);
        let action = read_payment_move(&g, 0, &command, &g.players[0].pool, &[]).unwrap();
        g.act(0, action).unwrap();
        assert!(g.obj(source).tapped);
        assert_eq!((g.players[0].pool.colors[3], g.players[0].pool.colors[4]), (0, 1));
    }

    #[test]
    fn r605_3_recursive_payment_wire_retains_child_x_life_and_printed_indices() {
        // 107.3a, 118.13a, 602.2b: X=1 and paying G/P with 2 life
        // leave one generic mana, supplied by the Forest's nested activation.
        let defs=vec![CardDef::new("Spell","{W}","Sorcery","You gain 1 life.",None).unwrap(),
            CardDef::new("Forest","","Basic Land — Forest","",None).unwrap(),
            CardDef::new("Filter","","Land","{X}{G/P}, {T}: Add {W}.",None).unwrap()];
        let mut g=Game::new(defs,2,20);
        let card=g.put(0,0,Zone::Hand);let forest=g.put(0,1,Zone::Battlefield);let filter=g.put(0,2,Zone::Battlefield);
        let command=json!({"kind":"cast","card":card.0,"plan":[{"source":filter.0,"ability":0,"x":1,"life":[true],
            "plan":[{"source":forest.0,"ability":0,"mana":{}}],"mana":{"G":1}}],"mana":{"W":1}});
        let before=format!("{g:?}");
        let quote=payment_quote(&g,0,&command).unwrap();
        assert_eq!(quote["complete"],true,"{quote}");
        assert_eq!(quote["payments"][1]["symbol_indices"],json!([0]));
        assert_eq!(quote["payments"][1]["path"],json!([0]));
        assert_eq!(quote["productions"][0]["path"],json!([0,0]));
        assert_eq!(format!("{g:?}"),before);
        g.act(0,read_payment_move(&g,0,&command,&g.players[0].pool,&[]).unwrap()).unwrap();
        assert_eq!(g.players[0].life,18);
        assert_eq!(g.players[0].pool.total(),0);
    }

    #[test]
    fn r106_6_recursive_payment_wire_selects_generated_lots_at_each_scoped_boundary() {
        // CR 106.6: C restricted to ability costs pays the filter, whose G
        // restricted to creature spells pays the Bear and leaves one G.
        let defs=vec![CardDef::new("Bear","{G}","Creature — Bear","",Some((2,2))).unwrap(),
            CardDef::new("Source","","Land","{T}: Add {C}. Spend this mana only to activate abilities.",None).unwrap(),
            CardDef::new("Filter","","Land","{1}, {T}: Add {G}{G}. Spend this mana only to cast a creature spell.",None).unwrap()];
        let mut g=Game::new(defs,2,20);
        let card=g.put(0,0,Zone::Hand);let source=g.put(0,1,Zone::Battlefield);let filter=g.put(0,2,Zone::Battlefield);
        let command=json!({"kind":"cast","card":card.0,"plan":[{"source":filter.0,"ability":0,
            "plan":[{"source":source.0,"ability":0,"mana":{}}],"mana":{"restricted_after":[0]}}],"mana":{"restricted_after":[0]}});
        let quote=payment_quote(&g,0,&command).unwrap();
        assert_eq!(quote["complete"],true,"{quote}");
        assert_eq!(quote["payments"][1]["selected"]["restricted"][0]["source"],source.0);
        assert_eq!(quote["payments"][2]["selected"]["restricted"][0]["source"],filter.0);
        g.act(0,read_payment_move(&g,0,&command,&g.players[0].pool,&[]).unwrap()).unwrap();
        assert_eq!(g.players[0].pool.restricted.len(),1);
        assert_eq!(g.players[0].pool.restricted[0].source,filter);
    }

    #[test]
    fn r106_6_player_labels_authenticate_nested_and_outer_post_production_lots() {
        // CR 106.6 / 601.2h / 605.3: the first source's C pays only the
        // filter's activation; one of the filter's two G pays the creature spell.
        let defs = vec![CardDef::new("Bear", "{G}", "Creature — Bear", "", Some((2,2))).unwrap(),
            CardDef::new("Ability source", "", "Land", "{T}: Add {C}. Spend this mana only to activate abilities.", None).unwrap(),
            CardDef::new("Creature filter", "", "Land", "{1}, {T}: Add {G}{G}. Spend this mana only to cast a creature spell.", None).unwrap()];
        for def in &defs { check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let card = g.put(0, 0, Zone::Hand);
        let first = g.put(0, 1, Zone::Battlefield);
        let filter = g.put(0, 2, Zone::Battlefield);
        let mut host = Some(Simulation::new(g));
        let (mut session, mut delivery, faces) = (1, Delivery::default(), Catalog::new(BTreeMap::new()));
        let prompt = dispatch(&faces, &mut host, &mut session, &mut delivery,
            &json!({"cmd":"prompt","seat":"p0"})).unwrap();
        let labels = host.as_ref().unwrap().labels(0).unwrap();
        let label = |id| Label::Object(labels.number(id).unwrap()).to_string();
        let mut answer = json!({"kind":"cast","card":label(card),"plan":[
            {"source":label(first),"ability":0},
            {"source":label(filter),"ability":0,"mana":{"restricted_after":[0]}}],
            "mana":{"restricted_after":[9]}});
        let request = |answer| json!({"cmd":"answer","seat":"p0","session_id":1,
            "request_id":prompt["request_id"],"answer":answer});
        let before = format!("{:?}", host.as_ref().unwrap().game().unwrap());
        // Lot 9 does not exist in the outer pool after the two authored steps.
        // Reject its identity, rather than accepting an unrelated payment/label failure.
        assert_eq!(dispatch(&faces, &mut host, &mut session, &mut delivery,
            &request(answer.clone())).unwrap_err(),
            "no such restricted mana in the selected pool boundary");
        assert_eq!(format!("{:?}", host.as_ref().unwrap().game().unwrap()), before);
        answer["mana"]["restricted_after"] = json!([0]);
        dispatch(&faces, &mut host, &mut session, &mut delivery, &request(answer)).unwrap();
        let live = host.as_ref().unwrap().game().unwrap();
        assert_eq!(live.stack.len(), 1);
        assert!(live.obj(first).tapped && live.obj(filter).tapped);
        assert_eq!(live.players[0].pool.total(), 1);
        assert_eq!(live.players[0].pool.restricted[0].mana, ir::mana::ManaSymbol::Color(ir::Color::Green));
    }

    #[test]
    fn r106_6_nested_restricted_selection_uses_the_child_boundary_after_earlier_steps() {
        let defs = vec![CardDef::new("Gift", "{G}", "Sorcery", "You gain 1 life.", None).unwrap(),
            CardDef::new("Restricted source", "", "Land", "{T}: Add {C}. Spend this mana only to activate abilities.", None).unwrap(),
            CardDef::new("Filter", "", "Land", "{1}, {T}: Add {G}{G}.", None).unwrap()];
        for def in &defs { check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let card = g.put(0, 0, Zone::Hand); let first = g.put(0, 1, Zone::Battlefield); let filter = g.put(0, 2, Zone::Battlefield);
        let before = format!("{g:?}");
        for selector in ["restricted", "restricted_after"] {
            let mut command = json!({"kind":"cast","card":card.0,"plan":[
                {"source":first.0,"ability":0}, {"source":filter.0,"ability":0,"mana":{}}], "mana":{"G":1}});
            command["plan"][1]["mana"][selector] = json!([0]);
            let q = payment_quote(&g, 0, &command).unwrap();
            assert_eq!(q["complete"], true, "{q}");
            assert_eq!(q["payments"][1]["selected"]["restricted"][0]["source"], first.0);
            assert_eq!(q["payments"][1]["left"]["restricted"], json!([]));
            assert_eq!(q["payments"][2]["left"]["G"], 1);
            assert_eq!(format!("{g:?}"), before);
        }
    }
}


#[cfg(test)]
mod retained_discard_tests {
    use super::*;

    #[test]
    fn r616_1_native_discard_cost_exposes_discard_event_and_exact_destination_identities() {
        // 616.1e–f, 118.11: the caller chooses Colossus or RIP before the discard cost commits.
        let defs = vec![
            CardDef::new("Darksteel Colossus", "{11}", "Artifact Creature — Golem", "Trample\nIndestructible\nIf Darksteel Colossus would be put into a graveyard from anywhere, reveal Darksteel Colossus and shuffle it into its owner's library instead.", Some((11,11))).unwrap(),
            CardDef::new("Rest in Peace", "{1}{W}", "Enchantment", "When this enchantment enters, exile all graveyards.\nIf a card or token would be put into a graveyard from anywhere, exile it instead.", None).unwrap(),
            CardDef::new("Thrill of Possibility", "{1}{R}", "Instant", "As an additional cost to cast this spell, discard a card.\nDraw two cards.", None).unwrap(),
        ];
        for def in &defs { check(def).unwrap(); }
        let mut g = Game::new(defs, 2, 20);
        let rip = g.put(0, 1, Zone::Battlefield);
        let card = g.put(0, 0, Zone::Hand);
        let thrill = g.put(0, 2, Zone::Hand);
        g.add_mana(0, ir::mana::ManaSymbol::Color(ir::Color::Red));
        g.add_mana(0, ir::mana::ManaSymbol::Colorless);
        let command = json!({"kind":"cast","card":thrill.0,"targets":[],"paying":[[card.0]],"mana":{"C":1,"R":1}});
        let action = read_move(&command, &g.players[0].pool).unwrap();
        g.act(0, action).unwrap();
        let q = asked(&mut g);
        assert_eq!(q["kind"], "replacement");
        assert_eq!(q["event"], "discard");
        assert_eq!(q["player"], 0);
        assert_eq!(q["on"], json!({"object":card.0}));
        assert_eq!(q["optional"], false);
        assert_eq!(q["among"], json!([
            {"source":card.0,"ability":2,"part":0,"name":"Darksteel Colossus"},
            {"source":rip.0,"ability":1,"part":0,"name":"Rest in Peace"},
        ]));
        let before = format!("{g:?}");
        let wrong = read_move(&json!({"kind":"choose","answer":{"replacement":{"source":rip.0,"ability":0,"part":0}}}), &Pool::default()).unwrap();
        assert_eq!(g.act(0, wrong), Err("choose one of the replacement effects that apply (616.1)".into()));
        assert_eq!(format!("{g:?}"), before);
        let choice = read_move(&json!({"kind":"choose","answer":{"replacement":{"source":rip.0,"ability":1,"part":0}}}), &Pool::default()).unwrap();
        g.act(0, choice).unwrap();
        assert_eq!(g.exile.len(), 1);
        assert_eq!(g.obj(g.exile[0]).chars.name, "Darksteel Colossus");
        assert_eq!(g.stack.len(), 1);
        assert_eq!(g.obj(g.stack[0]).chars.name, "Thrill of Possibility");
        assert_eq!(g.players[0].pool.total(), 0);
        assert!(g.decision.is_none());
        assert_eq!(g.log.iter().filter(|event| matches!(event, game::Event::Discarded { .. })).count(), 1);
    }
}
