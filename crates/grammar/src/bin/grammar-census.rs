//! A census of the IR over the corpus: which constructors each card's reading uses, and what
//! kind of thing fills each slot.
//!
//! `cargo run --profile fast -p grammar --bin grammar-census -- [OUT.jsonl]`
//!
//! The round trip cannot tell a rule that captures a structure from a rule that memorizes one
//! sentence: both print their text back. But a memorized sentence needs a constructor of its own
//! to print from, so the number of cards each constructor serves can. This walks the value of
//! every accepted row and records:
//!
//! - its constructors: enum variants (`Action::Destroy`) and boolean fields that are set
//!   (`Replacement.instead_first`);
//! - its slots: for each place a selector or set stands, the domain of what stands there
//!   (`Action::Destroy=Permanent`, `Action::Attach.to=Permanent`), and for each action the domain
//!   of its clause's subject (`Action::Draw<Player`, `Action::Draw<none` for an imperative). The
//!   domains real cards put in a slot are that slot's type; a rare one is a reading to check.
//!
//! One JSON line per face: the face, whether every row was read, and the constructors and slots
//! of the rows that were. `scripts/census.py` reads it.

#![forbid(unsafe_code)]

use grammar::ability::abilities;
use grammar::kernel::Outcome;
use oracle::classify::{Kind as Row, classify};
use oracle::corpus;
use oracle::normalize::normalize;
use oracle::token::tokenize;
use serde::Serialize;
use serde::ser::{self, Impossible};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::fmt;
use std::process::ExitCode;

/// What the walk has passed through to get where it is.
#[derive(Debug, Clone, PartialEq)]
enum Frame {
    /// `Enum::Variant`.
    Variant(&'static str, &'static str),
    Struct(&'static str),
    Field(&'static str),
}

/// The enums a noun phrase is made of: a slot is named by the nearest variant outside them.
const NOUN_PHRASE: [&str; 9] = ["Quant", "Ref", "Filter", "Atom", "Count", "Anaphor", "Slice", "Subject", "Who"];

#[derive(Default)]
struct Seen {
    ctors: RefCell<BTreeSet<String>>,
    slots: RefCell<BTreeSet<String>>,
    path: RefCell<Vec<Frame>>,
    /// The clauses being walked, innermost last: the subject's domain and the actions so far.
    clauses: RefCell<Vec<(Option<String>, Vec<String>)>>,
}

impl Seen {
    fn push(&self, f: Frame) {
        if f == Frame::Struct("Clause") {
            self.clauses.borrow_mut().push((None, Vec::new()));
        }
        self.path.borrow_mut().push(f);
    }

    fn pop(&self) {
        if self.path.borrow_mut().pop() == Some(Frame::Struct("Clause"))
            && let Some((subject, actions)) = self.clauses.borrow_mut().pop()
        {
            let subject = subject.unwrap_or_else(|| "none".to_string());
            for a in actions {
                self.slots.borrow_mut().insert(format!("{a}<{subject}"));
            }
        }
    }

    fn variant(&self, name: &'static str, variant: &'static str) {
        self.ctors.borrow_mut().insert(format!("{name}::{variant}"));
        let path = self.path.borrow();
        // an action of the clause being walked: Clause.first/rest → Act.action
        if name == "Action"
            && matches!(path.as_slice(), [.., Frame::Struct("Act"), Frame::Field("action")])
            && let Some(c) = self.clauses.borrow_mut().last_mut()
        {
            c.1.push(format!("Action::{variant}"));
        }
        if name != "Domain" {
            return;
        }
        // the slot this domain fills: up to the nearest variant outside the noun phrase, through
        // exactly one set (a set inside another, "creatures you control", describes the outer one)
        let (mut sets, mut selectors, mut fields) = (0, 0, Vec::new());
        for f in path.iter().rev() {
            match f {
                Frame::Struct("ObjectSet") => sets += 1,
                Frame::Struct("Selector") => selectors += 1,
                Frame::Struct(_) => {}
                Frame::Field(k) => {
                    if sets == 1 && selectors <= 1 && !matches!(*k, "set" | "items" | "domain") {
                        fields.push(*k);
                    }
                }
                Frame::Variant(e, v) if !NOUN_PHRASE.contains(e) || matches!((*e, *v), ("Subject", _) | ("Who", _)) => {
                    if sets == 1 && selectors <= 1 {
                        if *e == "Subject" {
                            // the subject of the innermost clause
                            if let Some(c) = self.clauses.borrow_mut().last_mut()
                                && c.0.is_none()
                            {
                                c.0 = Some(variant.to_string());
                            }
                        } else if *e != "Who" {
                            fields.reverse();
                            let role = if fields.is_empty() { String::new() } else { format!(".{}", fields.join(".")) };
                            self.slots.borrow_mut().insert(format!("{e}::{v}{role}={variant}"));
                        }
                    }
                    if *e != "Who" {
                        return;
                    }
                }
                Frame::Variant(..) => {
                    if sets > 1 || selectors > 1 {
                        return;
                    }
                }
            }
        }
    }
}

/// A serializer that keeps no data, only the names of what it passes through.
struct Census<'a> {
    seen: &'a Seen,
    /// The field being written (`Struct.field`), recorded when its value is `true`.
    field: Option<String>,
}

#[derive(Debug)]
struct Never;

impl fmt::Display for Never {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("census error")
    }
}

impl std::error::Error for Never {}

impl ser::Error for Never {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Never
    }
}

struct Compound<'a> {
    seen: &'a Seen,
    /// The struct or variant whose fields these are; None for sequences and tuples.
    owner: Option<String>,
    /// Whether this compound pushed a frame, popped when it ends.
    framed: bool,
}

impl<'a> Compound<'a> {
    fn item<T: ?Sized + Serialize>(&self, value: &T) -> Result<(), Never> {
        value.serialize(Census { seen: self.seen, field: None })
    }
    fn named<T: ?Sized + Serialize>(&self, key: &'static str, value: &T) -> Result<(), Never> {
        let field = self.owner.as_ref().map(|o| format!("{o}.{key}"));
        self.seen.push(Frame::Field(key));
        let r = value.serialize(Census { seen: self.seen, field });
        self.seen.pop();
        r
    }
    fn finish(self) -> Result<(), Never> {
        if self.framed {
            self.seen.pop();
        }
        Ok(())
    }
}

macro_rules! ignore {
    ($($name:ident: $ty:ty),*) => {
        $(fn $name(self, _: $ty) -> Result<(), Never> { Ok(()) })*
    };
}

impl<'a> ser::Serializer for Census<'a> {
    type Ok = ();
    type Error = Never;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Impossible<(), Never>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    ignore!(serialize_i8: i8, serialize_i16: i16, serialize_i32: i32, serialize_i64: i64, serialize_u8: u8,
        serialize_u16: u16, serialize_u32: u32, serialize_u64: u64, serialize_f32: f32, serialize_f64: f64,
        serialize_char: char, serialize_str: &str, serialize_bytes: &[u8]);

    fn serialize_bool(self, v: bool) -> Result<(), Never> {
        if v && let Some(f) = self.field {
            self.seen.ctors.borrow_mut().insert(f);
        }
        Ok(())
    }
    fn serialize_none(self) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Never> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Never> {
        Ok(())
    }
    fn serialize_unit_variant(self, name: &'static str, _: u32, variant: &'static str) -> Result<(), Never> {
        self.seen.variant(name, variant);
        Ok(())
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, value: &T) -> Result<(), Never> {
        value.serialize(Census { seen: self.seen, field: None })
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Never> {
        self.seen.variant(name, variant);
        self.seen.push(Frame::Variant(name, variant));
        let r = value.serialize(Census { seen: self.seen, field: None });
        self.seen.pop();
        r
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Compound<'a>, Never> {
        Ok(Compound { seen: self.seen, owner: None, framed: false })
    }
    fn serialize_tuple(self, _: usize) -> Result<Compound<'a>, Never> {
        Ok(Compound { seen: self.seen, owner: None, framed: false })
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Compound<'a>, Never> {
        Ok(Compound { seen: self.seen, owner: None, framed: false })
    }
    fn serialize_tuple_variant(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, Never> {
        self.seen.variant(name, variant);
        self.seen.push(Frame::Variant(name, variant));
        Ok(Compound { seen: self.seen, owner: None, framed: true })
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Never> {
        Err(Never)
    }
    fn serialize_struct(self, name: &'static str, _: usize) -> Result<Compound<'a>, Never> {
        self.seen.push(Frame::Struct(name));
        Ok(Compound { seen: self.seen, owner: Some(name.to_string()), framed: true })
    }
    fn serialize_struct_variant(
        self,
        name: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, Never> {
        self.seen.variant(name, variant);
        self.seen.push(Frame::Variant(name, variant));
        Ok(Compound { seen: self.seen, owner: Some(format!("{name}::{variant}")), framed: true })
    }
}

impl ser::SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.item(value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

impl ser::SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.item(value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

impl ser::SerializeTupleStruct for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.item(value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

impl ser::SerializeTupleVariant for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Never> {
        self.item(value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

impl ser::SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Never> {
        self.named(key, value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

impl ser::SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = Never;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Never> {
        self.named(key, value)
    }
    fn end(self) -> Result<(), Never> {
        self.finish()
    }
}

/// The constructors and slots of one value.
fn walk<T: Serialize>(value: &T) -> (BTreeSet<String>, BTreeSet<String>) {
    let seen = Seen::default();
    value.serialize(Census { seen: &seen, field: None }).expect("the IR has no maps");
    (seen.ctors.into_inner(), seen.slots.into_inner())
}

fn main() -> ExitCode {
    let out = std::env::args().nth(1).unwrap_or_else(|| "target/census.jsonl".to_string());
    let faces = match corpus::load(&corpus::default_path()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot read the corpus {}: {e}", corpus::default_path().display());
            return ExitCode::FAILURE;
        }
    };
    let ab = abilities();
    let lines: Vec<Option<String>> = {
        use rayon::prelude::*;
        faces
            .par_iter()
            .map(|f| {
                let n = normalize(&f.oracle_text, &f.name, &f.face_name, &f.type_line);
                let (mut ctors, mut slots) = (BTreeSet::new(), BTreeSet::new());
                let (mut rows, mut read) = (0usize, 0usize);
                for b in &classify(&n) {
                    if b.kind == Row::Empty {
                        continue;
                    }
                    for (i, r) in b.rows.clone().enumerate() {
                        let toks = tokenize(&n.rows[r]);
                        if toks.is_empty() {
                            continue;
                        }
                        rows += 1;
                        let walked = if i == 0 {
                            match ab.ability.parse_all(&toks) {
                                Outcome::Accepted { value, .. } => Some(walk(&value)),
                                _ => None,
                            }
                        } else {
                            match ab.mode.parse_all(&toks) {
                                Outcome::Accepted { value, .. } => Some(walk(&value)),
                                _ => None,
                            }
                        };
                        // a row is read when it has one reading and no quoted ability left as text
                        if let Some((c, s)) = walked
                            && !c.contains("Granted::Quoted")
                        {
                            read += 1;
                            ctors.extend(c);
                            slots.extend(s);
                        }
                    }
                }
                (rows > 0).then(|| {
                    serde_json::json!({"oracle_id": f.oracle_id, "face": f.face_name, "rows": rows, "read": read,
                        "ok": rows == read, "ctors": ctors, "slots": slots})
                    .to_string()
                })
            })
            .collect()
    };
    let lines: Vec<String> = lines.into_iter().flatten().collect();
    let ok = lines.iter().filter(|l| l.contains("\"ok\":true")).count();
    if let Err(e) = std::fs::write(&out, lines.join("\n") + "\n") {
        eprintln!("cannot write {out}: {e}");
        return ExitCode::FAILURE;
    }
    println!("faces with text: {}  every row read: {ok} ({:.1}%)  -> {out}", lines.len(), 100.0 * ok as f64 / lines.len().max(1) as f64);
    ExitCode::SUCCESS
}
