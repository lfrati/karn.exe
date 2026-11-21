//! The grammar kernel: rules that parse **and** print.
//!
//! A [`P<T>`] reads tokens into a `T`, writes a `T` back as tokens, and generates a `T` at random.
//! All three are built together by the same combinators, so a rule cannot exist with only one of
//! them, and the corpus round trip `print(parse(row)) == row` is a proof rather than a formality.
//! Generation ([`P::sample`]) asks the opposite question of the same rules: whether what they
//! accept is card text at all. The design is
//! argentum's Assay kernel (`argentum/oracle-assay/.../syntax/Phrase.kt`) in typed Rust:
//!
//! - **All parses.** A rule returns every reading with where it ended. Memoized per (rule,
//!   position), so the cost stays polynomial. Two different complete readings of a span are an
//!   [`Outcome::Ambiguous`] — a grammar error, never resolved by picking one.
//! - **Canonical printing.** Where English has two spellings of one meaning, one rule is
//!   canonical and the other is an [`P::alias`] that parses but never prints. A row whose printed
//!   form differs but reparses to the same value is a VARIANT, not a round trip.
//! - **Never panics on input.** A rule that cannot read a span returns no parse; the furthest
//!   failure and what was expected there are the decline diagnostic.
//! - **Spacing is the printer's.** Parsing ignores spaces; printing puts one space between
//!   tokens except before `, . : ; ! ?` and clitics, unless a rule says [`P::tight`].
//! - **Sentence case is positional.** A literal word also matches its capitalized form at a
//!   sentence start (row start, or after `. ! ? : • — |`), and printing capitalizes there.
//! - **Untyped inside.** A `P<T>` is a typed handle on an untyped rule: values travel as
//!   `Rc<dyn Any>`, so the machinery (memo, sequences, alternatives, lists) is compiled once rather
//!   than once per value type, and only small packing functions are generic. The types are checked
//!   where rules are built; a value of the wrong type inside is a kernel bug and panics.

use oracle::token::{Kind, Token};
use std::any::Any;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock};

/// More readings than this for one rule at one position is reported as a decline (the grammar
/// needs factoring there), not silently truncated.
pub const PARSE_CAP: usize = 64;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

fn next_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

fn starts_upper(k: &Kind) -> bool {
    matches!(k, Kind::Word(w) if w.chars().next().is_some_and(char::is_uppercase))
}

/// Is token `i` of `toks` at a sentence start? Also a capitalized word after a comma: Oracle
/// capitalizes each part of a cost ("{2}, {T}, Sacrifice a creature"), which [`P::capital`] prints.
pub fn sentence_start(toks: &[Token], i: usize) -> bool {
    i == 0
        || matches!(toks[i - 1].kind, Kind::Punct('.' | '!' | '?' | ':' | '•' | '—' | '|'))
        || ends_sentence(&toks[i - 1].kind)
        || (toks[i - 1].kind == Kind::Punct(',') && starts_upper(&toks[i].kind))
}

/// Drop the final period of a quote that the sentence goes on after; a quote that ends in a
/// nested quote loses the nested one's (`… "Equipped creature has '{1}: … any target'" and …`).
fn drop_final_period(k: &mut Kind) {
    if let Kind::Quote { body, closed: true, .. } = k {
        match body.last_mut().map(|t| &mut t.kind) {
            Some(Kind::Punct('.')) => {
                body.pop();
            }
            Some(inner @ Kind::Quote { .. }) => drop_final_period(inner),
            _ => {}
        }
    }
}

/// A quoted ability whose own last sentence ends the outer one too (`… with "{T}: Add {G}." It's
/// still a land.`): Oracle never writes a lowercase word after `."`. So does a quote that ends
/// in a nested quote that ends one (`… gains "{2}, {T}: Create … token with '… you control.'"`),
/// as [`drop_final_period`] reads the nested one's period.
pub fn ends_sentence(k: &Kind) -> bool {
    matches!(k, Kind::Quote { body, closed: true, .. }
        if body.last().is_some_and(|t| matches!(t.kind, Kind::Punct('.' | '!')) || ends_sentence(&t.kind)))
}

/// Why a span was declined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Decline {
    NoParse,
    /// A rule produced more than [`PARSE_CAP`] readings at one position.
    ParseCap,
    /// A rule re-entered itself at the same position.
    LeftRecursion,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<T> {
    /// Exactly one value; `redundant` counts extra derivations of the same value.
    Accepted { value: T, redundant: usize },
    /// Two or more different values for the same span.
    Ambiguous(Vec<T>),
    /// No complete reading. `at` is the furthest token any rule failed at.
    Declined { at: usize, expected: Vec<String>, reason: Decline },
}


/// A parsed value, of the type its rule's `P<T>` says.
type V = Rc<dyn Any>;
/// Every reading of a rule at a position: the value and where it ended.
type Readings = Vec<(V, usize)>;

#[cold]
fn type_error() -> ! {
    panic!("a rule's value has the type of its rule")
}

fn erase<T: 'static>(t: T) -> V {
    Rc::new(t)
}

fn down<T: 'static>(v: &dyn Any) -> &T {
    v.downcast_ref::<T>().unwrap_or_else(|| type_error())
}

/// Per-parse state: the input, the memo table, the recursion guard, the furthest failure.
pub struct Cx<'t> {
    toks: &'t [Token],
    memo: HashMap<(u32, usize), Rc<Readings>>,
    active: HashSet<(u32, usize)>,
    furthest: usize,
    expected: BTreeSet<String>,
    flag: Option<Decline>,
}

impl<'t> Cx<'t> {
    pub fn new(toks: &'t [Token]) -> Cx<'t> {
        Cx { toks, memo: HashMap::new(), active: HashSet::new(), furthest: 0, expected: BTreeSet::new(), flag: None }
    }

    pub fn token(&self, i: usize) -> Option<&'t Token> {
        self.toks.get(i)
    }

    pub fn at_sentence_start(&self, i: usize) -> bool {
        i < self.toks.len() && sentence_start(self.toks, i)
    }

    pub fn fail(&mut self, at: usize, expected: &str) {
        if at > self.furthest {
            self.furthest = at;
            self.expected.clear();
        }
        if at == self.furthest && self.expected.len() < 24 {
            self.expected.insert(expected.to_string());
        }
    }
}

/// The printer's output: tokens with spacing already decided.
#[derive(Debug, Default, Clone)]
pub struct Out {
    pub toks: Vec<Token>,
    /// Indices of words printed capitalized wherever they stand ([`P::capital`]).
    pub caps: BTreeSet<usize>,
}

/// How a printed token is spaced from the one before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spacing {
    /// One space, except before `, . : ; ! ?` and clitics, and at the start.
    Default,
    /// No space.
    Tight,
}

impl Out {
    /// Undo printing back to `mark` tokens.
    pub fn rollback(&mut self, mark: usize) {
        self.toks.truncate(mark);
        self.caps.retain(|&i| i < mark);
    }

    pub fn push(&mut self, kind: Kind, spacing: Spacing) {
        let space = match spacing {
            Spacing::Tight => 0,
            Spacing::Default if self.toks.is_empty() => 0,
            Spacing::Default => match kind {
                Kind::Punct(',' | '.' | ':' | ';' | '!' | '?') | Kind::Clitic(_) => 0,
                _ => 1,
            },
        };
        self.toks.push(Token { kind, space_before: space });
    }

    /// The text, with sentence-initial words capitalized and each article agreeing with the word
    /// after it ("a" / "an"); `capitalize_first` says whether the first token is itself at a
    /// sentence start.
    pub fn text(&self, capitalize_first: bool) -> String {
        oracle::token::detokenize(&self.tokens(capitalize_first))
    }

    /// The tokens as [`Out::text`] spells them (for a quoted ability's body).
    pub fn tokens(&self, capitalize_first: bool) -> Vec<Token> {
        let mut toks = self.toks.clone();
        for i in 0..toks.len().saturating_sub(1) {
            if let Kind::Word(w) = &toks[i].kind
                && (w == "a" || w == "an")
            {
                let an = takes_an(&toks[i + 1].kind);
                toks[i].kind = Kind::Word(if an { "an" } else { "a" }.into());
            }
        }
        for i in 0..toks.len() {
            let start = if i == 0 { capitalize_first } else { sentence_start(&toks, i) } || self.caps.contains(&i);
            if start && let Kind::Word(w) = &mut toks[i].kind {
                *w = capitalize(w);
            }
        }
        toks
    }
}

/// Does this word take "an"? By sound, which spelling approximates: a vowel letter, except
/// "one…", "uni…", "use…", "eu…" (a one-shot, a Unicorn), plus "X" ("an X/X"); numbers said
/// with a vowel (8, 11, 18, 80–89, 800…).
pub fn takes_an(next: &Kind) -> bool {
    match next {
        Kind::Word(w) => {
            let l = w.to_lowercase();
            let vowel = l.starts_with(['a', 'e', 'i', 'o', 'u']);
            let consonant_sound = ["one", "uni", "use", "usu", "eu"].iter().any(|p| l.starts_with(p));
            (vowel && !consonant_sound) || l == "x"
        }
        Kind::Number { value, .. } => {
            let s = value.to_string();
            s.starts_with('8') || s == "11" || s == "18"
        }
        _ => false,
    }
}

pub fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// A rule's two halves, untyped.
trait Imp: Send + Sync {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings;
    fn print(&self, value: &dyn Any, out: &mut Out) -> bool;
    /// A value this rule can read, chosen at random; None when the choices made lead nowhere
    /// (a guard rejects them, or the depth or step budget runs out).
    fn sample(&self, g: &mut Gen) -> Option<V>;
}

struct Node {
    id: u32,
    name: String,
    canonical: bool,
    imp: Box<dyn Imp>,
}

type Rule = Arc<Node>;

fn make(name: String, canonical: bool, imp: Box<dyn Imp>) -> Rule {
    Arc::new(Node { id: next_id(), name, canonical, imp })
}

/// Every reading of `rule` starting at `pos`, memoized.
fn parse_rule(rule: &Node, cx: &mut Cx, pos: usize) -> Rc<Readings> {
    let key = (rule.id, pos);
    if let Some(hit) = cx.memo.get(&key) {
        return hit.clone();
    }
    if !cx.active.insert(key) {
        cx.flag = Some(Decline::LeftRecursion);
        return Rc::new(Vec::new());
    }
    let mut result = rule.imp.parse(cx, pos);
    cx.active.remove(&key);
    if result.len() > PARSE_CAP {
        cx.flag = Some(Decline::ParseCap);
        result.truncate(PARSE_CAP);
    }
    let rc = Rc::new(result);
    cx.memo.insert(key, rc.clone());
    rc
}

/// Print `value` with `rule`, or return false (leaving `out` unchanged) when it cannot.
fn print_rule(rule: &Node, value: &dyn Any, out: &mut Out) -> bool {
    if !rule.canonical {
        return false;
    }
    let mark = out.toks.len();
    let ok = rule.imp.print(value, out);
    if !ok {
        out.rollback(mark);
    }
    ok
}

/// The random choices of one generated value: a seeded stream, and budgets that keep a
/// recursive grammar from running away (a value deeper or larger than they allow is abandoned
/// and the alternatives next to it are tried).
pub struct Gen {
    state: u64,
    depth: usize,
    max_depth: usize,
    steps: usize,
    max_steps: usize,
}

impl Gen {
    pub fn new(seed: u64, max_depth: usize, max_steps: usize) -> Gen {
        Gen { state: seed, depth: 0, max_depth, steps: 0, max_steps }
    }

    /// SplitMix64.
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

/// A value `rule` can read, or None when this run of choices leads nowhere. An alias never
/// generates: only the spelling that prints.
fn sample_rule(rule: &Node, g: &mut Gen) -> Option<V> {
    if !rule.canonical || g.depth >= g.max_depth || g.steps >= g.max_steps {
        return None;
    }
    g.depth += 1;
    g.steps += 1;
    let v = rule.imp.sample(g);
    g.depth -= 1;
    v
}

/// The tokens a single-token rule ([`leaf`]) is offered when generating: a leaf's reader says
/// which of them it accepts. Quoted abilities are not among them (not generated yet).
fn sample_pool() -> &'static [Kind] {
    static POOL: OnceLock<Vec<Kind>> = OnceLock::new();
    POOL.get_or_init(|| {
        let mut pool: Vec<Kind> =
            [0, 1, 2, 3, 4, 5, 6, 7, 10, 20].iter().map(|n| Kind::Number { value: *n, text: n.to_string() }).collect();
        for w in ["X", "d6", "d20", "charge", "time", "stun", "oil", "loyalty", "Goblin", "Treasure", "grace"] {
            pool.push(Kind::Word(w.to_string()));
        }
        for s in ["W", "U", "B", "R", "G", "C", "S", "X", "0", "1", "2", "3", "W/U", "B/R", "2/W", "G/P", "T", "Q", "E", "P"] {
            pool.push(Kind::Symbol(s.to_string()));
        }
        pool.extend([Kind::Clitic("'s".into()), Kind::Clitic("'".into()), Kind::SelfRef, Kind::GrantingSelf]);
        pool
    })
}

/// A grammar rule producing `T`.
pub struct P<T> {
    rule: Rule,
    _t: PhantomData<fn() -> T>,
}

impl<T> Clone for P<T> {
    fn clone(&self) -> Self {
        P { rule: self.rule.clone(), _t: PhantomData }
    }
}

fn typed<T>(rule: Rule) -> P<T> {
    P { rule, _t: PhantomData }
}

/// Values a rule can carry: cloned out of the memo table, compared for ambiguity.
pub trait Val: Clone + PartialEq + Send + Sync + 'static {}
impl<T: Clone + PartialEq + Send + Sync + 'static> Val for T {}

// ---------------------------------------------------------------------- untyped rules

/// The same readings and printing under another name, or with a twist on printing.
struct Wrap {
    inner: Rule,
    kind: WrapKind,
}

enum WrapKind {
    Same,
    /// Parses, never prints.
    Alias,
    /// The first token must be a capitalized word (or no word), and prints capitalized.
    Capital,
    /// The first printed token takes no space.
    Tight,
    /// A sentence: quotes it goes on after drop their final period.
    Sentence,
}

impl Imp for Wrap {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        if let WrapKind::Capital = self.kind
            && let Some(k @ Kind::Word(_)) = cx.token(pos).map(|t| &t.kind)
            && !starts_upper(k)
        {
            cx.fail(pos, "a capitalized word");
            return Vec::new();
        }
        (*parse_rule(&self.inner, cx, pos)).clone()
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        let mark = out.toks.len();
        match self.kind {
            WrapKind::Alias => false,
            WrapKind::Same => print_rule(&self.inner, v, out),
            WrapKind::Capital => {
                let ok = print_rule(&self.inner, v, out);
                if ok {
                    out.caps.insert(mark);
                }
                ok
            }
            WrapKind::Tight => {
                let ok = print_rule(&self.inner, v, out);
                if ok && let Some(t) = out.toks.get_mut(mark) {
                    t.space_before = 0;
                }
                ok
            }
            WrapKind::Sentence => {
                if !print_rule(&self.inner, v, out) {
                    return false;
                }
                let last = out.toks.len().saturating_sub(1);
                for t in &mut out.toks[mark..last] {
                    drop_final_period(&mut t.kind);
                }
                true
            }
        }
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        match self.kind {
            WrapKind::Alias => None,
            _ => sample_rule(&self.inner, g),
        }
    }
}

struct NotBefore(Rule, Rule);

impl Imp for NotBefore {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let readings = parse_rule(&self.0, cx, pos);
        let mut out = Vec::new();
        for (v, e) in readings.iter() {
            if parse_rule(&self.1, cx, *e).is_empty() {
                out.push((v.clone(), *e));
            }
        }
        out
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        print_rule(&self.0, v, out)
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        sample_rule(&self.0, g)
    }
}

/// Taking a pair value apart for printing.
type Split = fn(&dyn Any) -> Option<(&dyn Any, &dyn Any)>;

/// How a sequence combines its two values.
enum Combine {
    /// Into a pair: `join` builds it, `split` takes it apart for printing.
    Pair { join: fn(&V, &V) -> V, split: Split },
    /// The first value (the second is `()`).
    Left,
    /// The second value (the first is `()`).
    Right,
}

struct Seq(Rule, Rule, Combine);

impl Imp for Seq {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut out = Vec::new();
        let first = parse_rule(&self.0, cx, pos);
        for (a, mid) in first.iter() {
            let second = parse_rule(&self.1, cx, *mid);
            if second.is_empty() {
                cx.fail(*mid, &self.1.name);
            }
            for (b, end) in second.iter() {
                let v = match &self.2 {
                    Combine::Pair { join, .. } => join(a, b),
                    Combine::Left => a.clone(),
                    Combine::Right => b.clone(),
                };
                out.push((v, *end));
            }
        }
        out
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        match &self.2 {
            Combine::Pair { split, .. } => {
                split(v).is_some_and(|(a, b)| print_rule(&self.0, a, out) && print_rule(&self.1, b, out))
            }
            Combine::Left => print_rule(&self.0, v, out) && print_rule(&self.1, &(), out),
            Combine::Right => print_rule(&self.0, &(), out) && print_rule(&self.1, v, out),
        }
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        let a = sample_rule(&self.0, g)?;
        let b = sample_rule(&self.1, g)?;
        Some(match &self.2 {
            Combine::Pair { join, .. } => join(&a, &b),
            Combine::Left => a,
            Combine::Right => b,
        })
    }
}

fn seq(a: &Rule, b: &Rule, combine: Combine) -> Rule {
    make(format!("{} {}", a.name, b.name), true, Box::new(Seq(a.clone(), b.clone(), combine)))
}

fn map_rule(inner: &Rule, to: To, from: From) -> Rule {
    make(inner.name.clone(), inner.canonical, Box::new(Map { inner: inner.clone(), to, from }))
}

fn join_pair<A: Val, B: Val>(a: &V, b: &V) -> V {
    erase((down::<A>(&**a).clone(), down::<B>(&**b).clone()))
}

fn split_pair<A: 'static, B: 'static>(v: &dyn Any) -> Option<(&dyn Any, &dyn Any)> {
    let (a, b) = v.downcast_ref::<(A, B)>()?;
    Some((a, b))
}

type To = Box<dyn Fn(&V) -> Option<V> + Send + Sync>;
type From = Box<dyn Fn(&dyn Any) -> Option<V> + Send + Sync>;

struct Map {
    inner: Rule,
    to: To,
    from: From,
}

impl Imp for Map {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        parse_rule(&self.inner, cx, pos).iter().filter_map(|(t, e)| (self.to)(t).map(|u| (u, *e))).collect()
    }
    fn print(&self, u: &dyn Any, out: &mut Out) -> bool {
        (self.from)(u).is_some_and(|t| print_rule(&self.inner, &*t, out))
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        sample_rule(&self.inner, g).and_then(|t| (self.to)(&t))
    }
}

/// Packing a list of values into the rule's `Vec<T>` and back.
#[derive(Clone, Copy)]
struct VecOf {
    collect: fn(&[V]) -> V,
    items: fn(&dyn Any) -> Option<Vec<&dyn Any>>,
}

fn collect_vec<T: Val>(items: &[V]) -> V {
    erase(items.iter().map(|v| down::<T>(&**v).clone()).collect::<Vec<T>>())
}

fn vec_items<T: 'static>(v: &dyn Any) -> Option<Vec<&dyn Any>> {
    Some(v.downcast_ref::<Vec<T>>()?.iter().map(|t| t as &dyn Any).collect())
}

fn vec_of<T: Val>() -> VecOf {
    VecOf { collect: collect_vec::<T>, items: vec_items::<T> }
}

/// Zero or more.
struct Many(Rule, VecOf);

impl Imp for Many {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut done = vec![((self.1.collect)(&[]), pos)];
        let mut frontier: Vec<(Vec<V>, usize)> = vec![(Vec::new(), pos)];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for (items, at) in frontier {
                for (t, e) in parse_rule(&self.0, cx, at).iter() {
                    if *e == at {
                        continue; // an empty item would repeat forever
                    }
                    let mut v = items.clone();
                    v.push(t.clone());
                    done.push(((self.1.collect)(&v), *e));
                    next.push((v, *e));
                }
            }
            frontier = next;
        }
        done
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        (self.1.items)(v).is_some_and(|items| items.into_iter().all(|t| print_rule(&self.0, t, out)))
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        // mostly none, sometimes one or two; an item that leads nowhere ends the run
        let mut items = Vec::new();
        while items.len() < 2 && g.below(3) == 0 {
            match sample_rule(&self.0, g) {
                Some(v) => items.push(v),
                None => break,
            }
        }
        Some((self.1.collect)(&items))
    }
}

/// One or more sentences, each ended by its period (see [`P::sentences`]).
struct Sentences(Rule, VecOf, bool);

impl Imp for Sentences {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut done = Vec::new();
        let mut frontier: Vec<(Vec<V>, usize)> = vec![(Vec::new(), pos)];
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for (items, at) in frontier {
                for (t, e) in parse_rule(&self.0, cx, at).iter() {
                    if *e == at {
                        continue;
                    }
                    // a quote ending in "." ends the sentence: nothing reads past it
                    if (at..*e - 1).any(|i| cx.token(i).is_some_and(|t| ends_sentence(&t.kind))) {
                        continue;
                    }
                    // a modal header ("Choose one —") has no period either
                    let quoted_end =
                        cx.token(*e - 1).is_some_and(|t| ends_sentence(&t.kind) || t.kind == Kind::Punct('—'));
                    let end = match cx.token(*e).map(|t| &t.kind) {
                        Some(Kind::Punct('.')) if !quoted_end => Some(*e + 1),
                        _ if quoted_end => Some(*e),
                        None if self.2 => Some(*e),
                        _ => None,
                    };
                    let Some(end) = end else {
                        cx.fail(*e, "\".\"");
                        continue;
                    };
                    let mut v = items.clone();
                    v.push(t.clone());
                    done.push(((self.1.collect)(&v), end));
                    next.push((v, end));
                }
            }
            frontier = next;
        }
        done
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        let Some(items) = (self.1.items)(v) else { return false };
        let n = items.len();
        n > 0
            && items.into_iter().enumerate().all(|(i, t)| {
                let ok = print_rule(&self.0, t, out);
                // a modal header's dash only ends the last sentence ("Choose three. You may
                // choose the same mode more than once.")
                if ok && i + 1 < n && out.toks.last().is_some_and(|t| t.kind == Kind::Punct('—')) {
                    out.toks.pop();
                }
                if ok && !out.toks.last().is_some_and(|t| ends_sentence(&t.kind) || t.kind == Kind::Punct('—')) {
                    out.push(Kind::Punct('.'), Spacing::Default);
                }
                ok
            })
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        let mut items = vec![sample_rule(&self.0, g)?];
        while items.len() < 3 && g.below(3) == 0 {
            match sample_rule(&self.0, g) {
                Some(v) => items.push(v),
                None => break,
            }
        }
        Some((self.1.collect)(&items))
    }
}

/// Two or more items joined by a conjunction, with the serial comma.
struct List {
    item: Rule,
    comma: Rule,
    conj: Rule,
    vec: VecOf,
}

impl Imp for List {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut done = Vec::new();
        let mut frontier: Vec<(Vec<V>, usize)> =
            parse_rule(&self.item, cx, pos).iter().map(|(t, e)| (vec![t.clone()], *e)).collect();
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for (items, at) in frontier {
                // final item: ", conj x" for three or more, "conj x" for two
                let before_conj: Vec<usize> = if items.len() >= 2 {
                    parse_rule(&self.comma, cx, at).iter().map(|(_, e)| *e).collect()
                } else {
                    vec![at]
                };
                for b in before_conj {
                    for (_, c) in parse_rule(&self.conj, cx, b).iter() {
                        for (t, e) in parse_rule(&self.item, cx, *c).iter() {
                            let mut v = items.clone();
                            v.push(t.clone());
                            done.push(((self.vec.collect)(&v), *e));
                        }
                    }
                }
                // another comma-separated item
                for (_, c) in parse_rule(&self.comma, cx, at).iter() {
                    for (t, e) in parse_rule(&self.item, cx, *c).iter() {
                        let mut v = items.clone();
                        v.push(t.clone());
                        next.push((v, *e));
                    }
                }
            }
            frontier = next;
        }
        done
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        let Some(items) = (self.vec.items)(v) else { return false };
        let n = items.len();
        if n < 2 {
            return false;
        }
        for (i, t) in items.into_iter().enumerate() {
            if i > 0 {
                if n > 2 && !print_rule(&self.comma, &(), out) {
                    return false;
                }
                if i == n - 1 && !print_rule(&self.conj, &(), out) {
                    return false;
                }
            }
            if !print_rule(&self.item, t, out) {
                return false;
            }
        }
        true
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        let n = if g.below(3) == 0 { 3 } else { 2 };
        let items = (0..n).map(|_| sample_rule(&self.item, g)).collect::<Option<Vec<V>>>()?;
        Some((self.vec.collect)(&items))
    }
}

/// Two or more items separated by commas only.
struct CommaList {
    item: Rule,
    rest: Rule,
    vec: VecOf,
}

impl Imp for CommaList {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut done = Vec::new();
        let mut frontier: Vec<(Vec<V>, usize)> =
            parse_rule(&self.item, cx, pos).iter().map(|(t, e)| (vec![t.clone()], *e)).collect();
        while !frontier.is_empty() {
            let mut next = Vec::new();
            for (items, at) in frontier {
                for (t, e) in parse_rule(&self.rest, cx, at).iter() {
                    let mut v = items.clone();
                    v.push(t.clone());
                    done.push(((self.vec.collect)(&v), *e));
                    next.push((v, *e));
                }
            }
            frontier = next;
        }
        done
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        let Some(items) = (self.vec.items)(v) else { return false };
        items.len() >= 2
            && print_rule(&self.item, items[0], out)
            && items[1..].iter().all(|t| print_rule(&self.rest, *t, out))
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        let mut items = vec![sample_rule(&self.item, g)?, sample_rule(&self.rest, g)?];
        if g.below(3) == 0 {
            items.push(sample_rule(&self.rest, g)?);
        }
        Some((self.vec.collect)(&items))
    }
}

struct Alt(Vec<Rule>);

impl Imp for Alt {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        let mut out = Vec::new();
        for a in &self.0 {
            out.extend(parse_rule(a, cx, pos).iter().cloned());
        }
        out
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        self.0.iter().any(|a| print_rule(a, v, out))
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        // every alternative is as likely as any other; one that leads nowhere gives way to the next
        let n = self.0.len();
        let start = g.below(n.max(1));
        (0..n).find_map(|i| sample_rule(&self.0[(start + i) % n], g))
    }
}

struct Empty;

impl Imp for Empty {
    fn parse(&self, _: &mut Cx, pos: usize) -> Readings {
        vec![(erase(()), pos)]
    }
    fn print(&self, _: &dyn Any, _: &mut Out) -> bool {
        true
    }
    fn sample(&self, _: &mut Gen) -> Option<V> {
        Some(erase(()))
    }
}

struct Lit(Vec<Token>, String);

impl Imp for Lit {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        for (i, l) in self.0.iter().enumerate() {
            let ok = cx.token(pos + i).is_some_and(|t| token_matches(&l.kind, &t.kind, cx.at_sentence_start(pos + i)));
            if !ok {
                cx.fail(pos + i, &self.1);
                return Vec::new();
            }
        }
        vec![(erase(()), pos + self.0.len())]
    }
    fn print(&self, _: &dyn Any, out: &mut Out) -> bool {
        push_tokens(&self.0, out);
        true
    }
    fn sample(&self, _: &mut Gen) -> Option<V> {
        Some(erase(()))
    }
}

fn push_tokens(toks: &[Token], out: &mut Out) {
    for (i, t) in toks.iter().enumerate() {
        if i == 0 {
            out.push(t.kind.clone(), Spacing::Default);
        } else {
            out.toks.push(t.clone());
        }
    }
}

type Read = Box<dyn Fn(&Kind) -> Option<V> + Send + Sync>;
type Write = Box<dyn Fn(&dyn Any) -> Option<Kind> + Send + Sync>;

struct Leaf {
    name: String,
    read: Read,
    write: Write,
}

impl Imp for Leaf {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        match cx.token(pos).and_then(|t| (self.read)(&t.kind)) {
            Some(v) => vec![(v, pos + 1)],
            None => {
                cx.fail(pos, &self.name);
                Vec::new()
            }
        }
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        match (self.write)(v) {
            Some(k) => {
                out.push(k, Spacing::Default);
                true
            }
            None => false,
        }
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        // a leaf reads one token: any token of the pool it accepts
        let pool = sample_pool();
        let accepted: Vec<V> = pool.iter().filter_map(|k| (self.read)(k)).collect();
        (!accepted.is_empty()).then(|| accepted[g.below(accepted.len())].clone())
    }
}

/// A rule resolved on first use.
struct Deferred(Box<dyn Fn() -> Rule + Send + Sync>);

impl Imp for Deferred {
    fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
        (*parse_rule(&(self.0)(), cx, pos)).clone()
    }
    fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
        print_rule(&(self.0)(), v, out)
    }
    fn sample(&self, g: &mut Gen) -> Option<V> {
        sample_rule(&(self.0)(), g)
    }
}

// ---------------------------------------------------------------------- the typed API

impl<T: Val> P<T> {
    fn wrap(&self, name: String, canonical: bool, kind: WrapKind) -> P<T> {
        typed(make(name, canonical, Box::new(Wrap { inner: self.rule.clone(), kind })))
    }

    pub fn name(&self) -> &str {
        &self.rule.name
    }

    /// Print `value`, or return false (leaving `out` unchanged) when this rule cannot express it.
    pub fn print(&self, value: &T, out: &mut Out) -> bool {
        print_rule(&self.rule, value, out)
    }

    /// Parse exactly `toks[start..end]` (the whole slice gives sentence-start context).
    pub fn parse_span(&self, toks: &[Token], start: usize, end: usize) -> Outcome<T> {
        let mut cx = Cx::new(&toks[..end]);
        let readings = parse_rule(&self.rule, &mut cx, start);
        let mut values: Vec<T> = Vec::new();
        let mut complete = 0;
        for (v, e) in readings.iter() {
            if *e == end {
                complete += 1;
                let v = down::<T>(&**v);
                if !values.contains(v) {
                    values.push(v.clone());
                }
            } else {
                cx.fail(*e, "end");
            }
        }
        if let Some(reason) = cx.flag {
            return Outcome::Declined { at: cx.furthest, expected: cx.expected.into_iter().collect(), reason };
        }
        match values.len() {
            0 => Outcome::Declined {
                at: cx.furthest,
                expected: cx.expected.into_iter().collect(),
                reason: Decline::NoParse,
            },
            1 => Outcome::Accepted { value: values.pop().expect("one"), redundant: complete - 1 },
            _ => Outcome::Ambiguous(values),
        }
    }

    /// Where the readings of this rule starting at `start` end (for tools that abstract text).
    pub fn ends(&self, toks: &[Token], start: usize) -> Vec<usize> {
        let mut cx = Cx::new(toks);
        parse_rule(&self.rule, &mut cx, start).iter().map(|(_, e)| *e).collect()
    }

    pub fn parse_all(&self, toks: &[Token]) -> Outcome<T> {
        self.parse_span(toks, 0, toks.len())
    }

    /// A value this rule reads, chosen at random from `g`'s stream: the rules run in a third
    /// direction. None when the choices led nowhere; try another seed.
    pub fn sample(&self, g: &mut Gen) -> Option<T> {
        sample_rule(&self.rule, g).map(|v| down::<T>(&*v).clone())
    }

    /// Print to text; `capitalize_first` when the value starts a sentence.
    pub fn print_text(&self, value: &T, capitalize_first: bool) -> Option<String> {
        let mut out = Out::default();
        self.print(value, &mut out).then(|| out.text(capitalize_first))
    }

    // ------------------------------------------------------------------ combinators

    pub fn named(self, name: impl Into<String>) -> P<T> {
        let canonical = self.rule.canonical;
        self.wrap(name.into(), canonical, WrapKind::Same)
    }

    /// An alternate spelling: parses, never prints.
    pub fn alias(self) -> P<T> {
        self.wrap(format!("{} (alias)", self.name()), false, WrapKind::Alias)
    }

    /// Capitalized wherever it stands: the first token must be a capitalized word (or not a word)
    /// and prints capitalized. For the parts of a cost after the first ("{T}, Sacrifice ~").
    pub fn capital(self) -> P<T> {
        self.wrap(self.name().to_string(), true, WrapKind::Capital)
    }

    /// Print the first token with no space before it ("Ward—Pay", "{2}{U}").
    pub fn tight(self) -> P<T> {
        self.wrap(self.name().to_string(), true, WrapKind::Tight)
    }

    /// One sentence: a quoted ability inside it keeps its final period only when it ends the
    /// sentence ("… gains \"{T}: Draw a card\" for as long as …" / "… gains \"{T}: Draw a card.\"").
    pub fn sentence(self) -> P<T> {
        self.wrap(self.name().to_string(), true, WrapKind::Sentence)
    }

    /// Only the readings that `guard` cannot continue: "the amount of life you gained" (the
    /// trigger's life) must not end right before "this turn", or "… gained this turn" would also
    /// read as that amount followed by a duration. Prints as `self`.
    pub fn not_before(&self, guard: &P<()>) -> P<T> {
        let canonical = self.rule.canonical;
        typed(make(self.name().to_string(), canonical, Box::new(NotBefore(self.rule.clone(), guard.rule.clone()))))
    }

    /// Sequence: `self` then `next`.
    pub fn then<U: Val>(&self, next: &P<U>) -> P<(T, U)> {
        typed(seq(&self.rule, &next.rule, Combine::Pair { join: join_pair::<T, U>, split: split_pair::<T, U> }))
    }

    /// `self` followed by a value-less rule (a literal).
    pub fn skip(&self, next: &P<()>) -> P<T> {
        typed(seq(&self.rule, &next.rule, Combine::Left))
    }

    /// Partial isomorphism: `to` may reject a reading, `from` may refuse to print a value.
    pub fn map<U: Val>(
        &self,
        to: impl Fn(T) -> Option<U> + Send + Sync + 'static,
        from: impl Fn(&U) -> Option<T> + Send + Sync + 'static,
    ) -> P<U> {
        // only the two adapters are generic; the node is built once for all
        typed(map_rule(
            &self.rule,
            Box::new(move |v: &V| to(down::<T>(&**v).clone()).map(erase)),
            Box::new(move |u: &dyn Any| from(down::<U>(u)).map(erase)),
        ))
    }

    /// Zero or more.
    pub fn many(&self) -> P<Vec<T>> {
        typed(make(format!("{}*", self.name()), true, Box::new(Many(self.rule.clone(), vec_of::<T>()))))
    }

    /// One or more sentences, each ended by "." — or by nothing when it ends in a quoted ability
    /// whose own "." ends it ([`ends_sentence`]) or in the dash of a modal header.
    pub fn sentences(&self) -> P<Vec<T>> {
        self.sentences_with(false)
    }

    /// [`P::sentences`], the last period optional: a quoted ability's body, which drops its period
    /// when the outer sentence goes on (`gains "{T}: Draw a card" for as long as …`).
    pub fn sentences_open(&self) -> P<Vec<T>> {
        self.sentences_with(true)
    }

    fn sentences_with(&self, open: bool) -> P<Vec<T>> {
        typed(make(format!("{}.+", self.name()), true, Box::new(Sentences(self.rule.clone(), vec_of::<T>(), open))))
    }

    /// Zero or one.
    pub fn opt(&self) -> P<Option<T>> {
        let none = empty().map(|()| Some(None), |o: &Option<T>| o.is_none().then_some(()));
        let some = self.map(|t| Some(Some(t)), |o: &Option<T>| o.clone());
        alt(format!("[{}]", self.name()), vec![some, none])
    }

    /// Two or more items joined by `conj`: "x or y", "x, y, or z" (with the serial comma).
    pub fn list(&self, conj: &str) -> P<Vec<T>> {
        let imp = List { item: self.rule.clone(), comma: lit(",").rule, conj: lit(conj).rule, vec: vec_of::<T>() };
        typed(make(format!("{} {conj} list", self.name()), true, Box::new(imp)))
    }

    /// Two or more items separated by commas only ("noncreature, nonland permanent").
    pub fn comma_list(&self) -> P<Vec<T>> {
        let rest = lit(",").then_keep(self).rule;
        let imp = CommaList { item: self.rule.clone(), rest, vec: vec_of::<T>() };
        typed(make(format!("{}, list", self.name()), true, Box::new(imp)))
    }
}

impl P<()> {
    /// A value-less rule followed by `next`, keeping `next`'s value.
    pub fn then_keep<U: Val>(&self, next: &P<U>) -> P<U> {
        typed(seq(&self.rule, &next.rule, Combine::Right))
    }

    /// This rule as the constant `value`.
    pub fn to<U: Val>(&self, value: U) -> P<U> {
        let v2 = value.clone();
        self.map(move |()| Some(value.clone()), move |u| (*u == v2).then_some(()))
    }
}

/// Alternation: every alternative's readings; prints with the first canonical alternative that
/// can express the value.
pub fn alt<T: Val>(name: impl Into<String>, alternatives: Vec<P<T>>) -> P<T> {
    typed(make(name.into(), true, Box::new(Alt(alternatives.into_iter().map(|a| a.rule).collect()))))
}

/// Matches nothing, always.
pub fn empty() -> P<()> {
    typed(make("ε".to_string(), true, Box::new(Empty)))
}

/// Pronouns a card uses for itself or another character instead of "it": "put a +1/+1 counter
/// on him", "return her to her owner's hand", "he gains flying", "damage to himself". They mean
/// the object "it" means (the rules have no gender for objects), so they are spellings of the
/// literal "it", "its" or "itself": read as it, printed as it, and reported as variants.
fn is_pronoun_of(literal: &str, word: &str) -> bool {
    match literal {
        "it" => matches!(word, "he" | "him" | "she" | "her"),
        "its" => matches!(word, "his" | "her"),
        "itself" => matches!(word, "himself" | "herself"),
        _ => false,
    }
}

/// Does input token `t` (at a sentence start or not) match literal token `l`?
fn token_matches(l: &Kind, t: &Kind, start: bool) -> bool {
    match (l, t) {
        (Kind::Word(a), Kind::Word(b)) => {
            a == b || (start && *b == capitalize(a)) || (is_pronoun_of(a, b) || (start && is_pronoun_of(a, &b.to_lowercase())))
        }
        (Kind::Number { value: a, .. }, Kind::Number { value: b, .. }) => a == b,
        (a, b) => a == b,
    }
}

/// Literal text: "up to", "target", ",", "—". Tokenized with the Oracle lexer.
pub fn lit(text: &str) -> P<()> {
    let toks = oracle::token::tokenize(text);
    typed(make(format!("\"{text}\""), true, Box::new(Lit(toks, format!("\"{text}\"")))))
}

/// A closed table of phrases: each entry's text (one or more tokens) reads as its value. Prints a
/// value with its first entry. Lookup is by first token, so a table of hundreds of subtypes
/// costs one hash probe per position.
pub fn table<T: Val + std::hash::Hash + Eq>(name: &str, entries: Vec<(String, T)>) -> P<T> {
    struct Table<T> {
        name: String,
        by_first: HashMap<String, Vec<(Vec<Token>, T)>>,
        print: HashMap<T, Vec<Token>>,
        /// The distinct values, in entry order (to sample from).
        values: Vec<T>,
    }
    fn key(k: &Kind) -> String {
        match k {
            Kind::Word(w) => w.to_lowercase(),
            other => format!("{other:?}"),
        }
    }
    impl<T: Val + std::hash::Hash + Eq> Imp for Table<T> {
        fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
            let Some(first) = cx.token(pos) else {
                cx.fail(pos, &self.name);
                return Vec::new();
            };
            let mut out = Vec::new();
            if let Some(cands) = self.by_first.get(&key(&first.kind)) {
                for (toks, v) in cands {
                    let ok = toks.iter().enumerate().all(|(i, l)| {
                        cx.token(pos + i).is_some_and(|t| token_matches(&l.kind, &t.kind, cx.at_sentence_start(pos + i)))
                    });
                    if ok {
                        out.push((erase(v.clone()), pos + toks.len()));
                    }
                }
            }
            if out.is_empty() {
                cx.fail(pos, &self.name);
            }
            out
        }
        fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
            let Some(toks) = self.print.get(down::<T>(v)) else {
                return false;
            };
            push_tokens(toks, out);
            true
        }
        fn sample(&self, g: &mut Gen) -> Option<V> {
            (!self.values.is_empty()).then(|| erase(self.values[g.below(self.values.len())].clone()))
        }
    }
    let mut by_first: HashMap<String, Vec<(Vec<Token>, T)>> = HashMap::new();
    let mut print = HashMap::new();
    let mut values = Vec::new();
    for (text, v) in entries {
        let toks = oracle::token::tokenize(&text);
        assert!(!toks.is_empty(), "empty table entry in {name}");
        if !print.contains_key(&v) {
            print.insert(v.clone(), toks.clone());
            values.push(v.clone());
        }
        by_first.entry(key(&toks[0].kind)).or_default().push((toks, v));
    }
    typed(make(name.to_string(), true, Box::new(Table { name: name.to_string(), by_first, print, values })))
}

/// A single token read by a function: numbers, symbols. `print` gives the token back.
pub fn leaf<T: Val>(
    name: &str,
    read: impl Fn(&Kind) -> Option<T> + Send + Sync + 'static,
    write: impl Fn(&T) -> Option<Kind> + Send + Sync + 'static,
) -> P<T> {
    let read: Read = Box::new(move |k| read(k).map(erase));
    let write: Write = Box::new(move |v| write(down::<T>(v)));
    typed(make(name.to_string(), true, Box::new(Leaf { name: name.to_string(), read, write })))
}

/// A card name: a greedy run of capitalized words, with lowercase "of", "the", "and" inside when
/// a capitalized word follows ("Relentless Rats", "Hour of Need"). Greedy, so one reading.
pub fn name_run() -> P<String> {
    struct NameRun;
    fn cap(t: Option<&Token>) -> Option<&str> {
        match t.map(|t| &t.kind) {
            Some(Kind::Word(w)) if w.chars().next().is_some_and(char::is_uppercase) => Some(w),
            _ => None,
        }
    }
    impl Imp for NameRun {
        fn parse(&self, cx: &mut Cx, pos: usize) -> Readings {
            // a card name is one of a closed set (CR 201.1): the longest one starting here, which
            // may hold a comma or an apostrophe that a run of capitalized words cannot spell
            if let Some(n) = oracle::names::longest(&cx.toks[pos.min(cx.toks.len())..])
                && n > 1
            {
                let name = oracle::token::detokenize(&cx.toks[pos..pos + n]);
                return vec![(erase(name.trim_start().to_string()), pos + n)];
            }
            let mut words: Vec<&str> = Vec::new();
            let mut i = pos;
            while let Some(w) = cap(cx.token(i)) {
                words.push(w);
                i += 1;
                // "of"/"the" continue the name only before another capitalized word; an "and"
                // joins two names (a coordination, 2.6)
                let mut j = i;
                while let Some(Kind::Word(small)) = cx.token(j).map(|t| &t.kind)
                    && matches!(small.as_str(), "of" | "the")
                {
                    j += 1;
                }
                if j > i && cap(cx.token(j)).is_some() {
                    for k in i..j {
                        if let Some(Kind::Word(small)) = cx.token(k).map(|t| &t.kind) {
                            words.push(small);
                        }
                    }
                    i = j;
                }
            }
            if words.is_empty() {
                cx.fail(pos, "card name");
                return Vec::new();
            }
            vec![(erase(words.join(" ")), i)]
        }
        fn print(&self, v: &dyn Any, out: &mut Out) -> bool {
            let v = down::<String>(v);
            for t in oracle::token::tokenize(v) {
                out.push(t.kind, Spacing::Default);
            }
            !v.is_empty()
        }
        fn sample(&self, g: &mut Gen) -> Option<V> {
            const NAMES: [&str; 4] = ["Grizzly Bears", "Hour of Need", "Relentless Rats", "Shock"];
            Some(erase(NAMES[g.below(NAMES.len())].to_string()))
        }
    }
    typed(make("card name".to_string(), true, Box::new(NameRun)))
}

/// A rule defined later, for recursion: `let s = forward(); … s.define(rule);`.
pub struct Forward<T>(Arc<OnceLock<Rule>>, P<T>);

impl<T: Val> Forward<T> {
    pub fn rule(&self) -> P<T> {
        self.1.clone()
    }

    pub fn define(&self, rule: P<T>) {
        assert!(self.0.set(rule.rule).is_ok(), "forward rule defined twice");
    }
}

/// A rule found on first use: breaks cycles between grammars built once in statics (a keyword's
/// cost contains actions, and actions grant keywords).
pub fn lazy<T: Val>(name: &str, get: fn() -> P<T>) -> P<T> {
    typed(make(name.to_string(), true, Box::new(Deferred(Box::new(move || get().rule)))))
}

pub fn forward<T: Val>(name: &str) -> Forward<T> {
    let cell: Arc<OnceLock<Rule>> = Arc::new(OnceLock::new());
    let c = cell.clone();
    let rule = typed(make(
        name.to_string(),
        true,
        Box::new(Deferred(Box::new(move || c.get().expect("forward rule defined").clone()))),
    ));
    Forward(cell, rule)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oracle::token::tokenize;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    enum Q {
        Target,
        UpTo(u32),
    }

    fn number() -> P<u32> {
        alt(
            "number",
            vec![
                table("number word", vec![("one".into(), 1), ("two".into(), 2), ("three".into(), 3)]),
                leaf(
                    "digits",
                    |k| match k {
                        Kind::Number { value, .. } => Some(*value),
                        _ => None,
                    },
                    |n| Some(Kind::Number { value: *n, text: n.to_string() }),
                )
                .alias(),
            ],
        )
    }

    fn quant() -> P<Q> {
        alt(
            "quantifier",
            vec![
                lit("target").to(Q::Target),
                lit("up to").then_keep(&number()).skip(&lit("target")).map(
                    |n| Some(Q::UpTo(n)),
                    |q| match q {
                        Q::UpTo(n) => Some(*n),
                        _ => None,
                    },
                ),
            ],
        )
    }

    #[test]
    fn parse_and_print() {
        let toks = tokenize("Up to two target");
        let Outcome::Accepted { value, .. } = quant().parse_all(&toks) else { panic!() };
        assert_eq!(value, Q::UpTo(2));
        assert_eq!(quant().print_text(&value, true).as_deref(), Some("Up to two target"));
        assert_eq!(quant().print_text(&value, false).as_deref(), Some("up to two target"));
    }

    #[test]
    fn alias_parses_but_prints_canonical() {
        let toks = tokenize("up to 2 target");
        let Outcome::Accepted { value, .. } = quant().parse_all(&toks) else { panic!() };
        assert_eq!(quant().print_text(&value, false).as_deref(), Some("up to two target"));
    }

    #[test]
    fn capital_only_at_sentence_start() {
        let toks = tokenize("draw. Target");
        let r = lit("target").parse_span(&toks, 2, 3);
        assert!(matches!(r, Outcome::Accepted { .. }));
        let toks = tokenize("draw Target");
        assert!(matches!(lit("target").parse_span(&toks, 1, 2), Outcome::Declined { .. }));
    }

    #[test]
    fn lists_and_ambiguity() {
        let color = table("color", vec![("red".into(), 'r'), ("green".into(), 'g'), ("blue".into(), 'u')]);
        let l = color.list("or");
        for (text, n) in [("red or green", 2), ("red, green, or blue", 3)] {
            let Outcome::Accepted { value, .. } = l.parse_all(&tokenize(text)) else { panic!("{text}") };
            assert_eq!(value.len(), n);
            assert_eq!(l.print_text(&value, false).as_deref(), Some(text));
        }
        // two different readings of the same span
        let a = alt("amb", vec![lit("red").to(1), lit("red").to(2)]);
        assert!(matches!(a.parse_all(&tokenize("red")), Outcome::Ambiguous(_)));
    }

    /// CR 201.1: a name is a run of capitalized words or one of the card names; a known name
    /// holds the comma, apostrophe and hyphen a run cannot spell, and nothing else does.
    #[test]
    fn r201_1_card_names() {
        let name = name_run();
        for text in ["Relentless Rats", "Sam, Loyal Attendant", "God-Pharaoh's Gift", "Nissa's Chosen", "Hall of the Bandit Lord"]
        {
            let Outcome::Accepted { value, .. } = name.parse_all(&tokenize(text)) else { panic!("{text}") };
            assert_eq!(value, text);
            assert_eq!(name.print_text(&value, false).as_deref(), Some(text), "{text}");
        }
        // "Sam" alone is a name; a comma and a lowercase word after it are not part of it
        assert!(matches!(name.parse_all(&tokenize("Sam, loyal attendant")), Outcome::Declined { .. }));
        let r = name.parse_span(&tokenize("Dovin, Architect of Law, reveal"), 0, 5);
        assert!(matches!(r, Outcome::Accepted { .. }));
    }

    /// Objects have no gender in the rules: a character's "he", "him", "his" read as "it", "its"
    /// and print as them (a variant), and the pronoun of one word never stands for another's.
    #[test]
    fn pronouns_are_aliases_of_it() {
        for (text, lit_text, ok) in [
            ("him", "it", true),
            ("her", "it", true),
            ("he", "it", true),
            ("his", "its", true),
            ("her", "its", true),
            ("himself", "itself", true),
            ("his", "it", false),
            ("him", "its", false),
            ("he", "its", false),
            ("them", "it", false),
        ] {
            let r = lit(lit_text).parse_all(&tokenize(text));
            assert_eq!(matches!(r, Outcome::Accepted { .. }), ok, "{text} as {lit_text}");
        }
        assert_eq!(lit("it").print_text(&(), false).as_deref(), Some("it"));
    }

    #[test]
    fn declines_report_furthest() {
        let r = quant().parse_all(&tokenize("up to two creatures"));
        let Outcome::Declined { at, expected, .. } = r else { panic!() };
        assert_eq!(at, 3);
        assert!(expected.contains(&"\"target\"".to_string()), "{expected:?}");
    }

    #[test]
    fn recursion_through_forward() {
        // nested parentheses-free recursion: "very very good"
        let f = forward::<u32>("adj");
        let rule = alt(
            "adj",
            vec![lit("good").to(0u32), lit("very").then_keep(&f.rule()).map(|n| Some(n + 1), |n| n.checked_sub(1))],
        );
        f.define(rule.clone());
        let Outcome::Accepted { value, .. } = rule.parse_all(&tokenize("very very good")) else { panic!() };
        assert_eq!(value, 2);
        assert_eq!(rule.print_text(&2, false).as_deref(), Some("very very good"));
    }
}
