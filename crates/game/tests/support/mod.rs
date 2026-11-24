//! Shared test support: boards described by card names, drivers for the usual moves, and
//! expected outcomes, either as an explicit next state ([`State`]) or as properties
//! ([`Expect`]). `rules.rs` includes it once; a test file uses it with
//! `use crate::support;` and `use support::*;`.
#![allow(dead_code, unused_imports)]
pub use game::legal::Asked;
pub use game::state::{Answer, Choice, Decision};
pub use game::{CardDef, Game, Move, ObjId, Target, Zone};
pub use ir::Part;
pub mod cite;
pub(crate) use cite::cite;
pub use cite::Refusal;

/// Exact unrestricted mana chosen by a fixture: W/U/B/R/G/C units or `{3}{U}`.
/// Numeric amounts mean that many colorless units, never a choice of generic spending.
pub fn mana_pool(units: &str) -> game::state::Pool {
    if units.starts_with('{') {
        let mut pool = game::state::Pool::default();
        for part in units.split('{').skip(1) {
            let symbol = part.strip_suffix('}').expect("closed mana amount");
            if let Ok(amount) = symbol.parse::<u32>() {
                pool.colorless += amount;
            } else {
                let unit = mana_pool(symbol);
                pool.colorless += unit.colorless;
                for (to, amount) in pool.colors.iter_mut().zip(unit.colors) { *to += amount; }
            }
        }
        return pool;
    }
    let mut pool = game::state::Pool::default();
    for unit in units.chars() {
        match unit {
            'W' => pool.colors[0] += 1,
            'U' => pool.colors[1] += 1,
            'B' => pool.colors[2] += 1,
            'R' => pool.colors[3] += 1,
            'G' => pool.colors[4] += 1,
            'C' => pool.colorless += 1,
            _ => panic!("expected a W/U/B/R/G/C mana unit, got {unit}"),
        }
    }
    pool
}

/// Attach the fixture's chosen spending without inspecting a game or its cost.
pub fn paid(units: &str, action: Move) -> Move {
    Move::Paying(mana_pool(units), Box::new(action))
}

/// The game refused this for `reason`, as `cite!(..; refused(reason): ..)` checks it, where
/// no rule is credited: a simulation's fault, stale request or wrong seat is no refusal.
#[track_caller]
pub fn refused<V, E: Refusal>(result: Result<V, E>, reason: &str) { cite::refused(result, reason, &[]) }
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn faces() -> &'static Vec<oracle::corpus::Face> {
    static FACES: OnceLock<Vec<oracle::corpus::Face>> = OnceLock::new();
    FACES.get_or_init(|| oracle::corpus::load(&oracle::corpus::default_path()).unwrap())
}

/// A card by its Oracle name as the grammar reads it, whether or not the engine runs it.
pub fn oracle_read(name: &str) -> CardDef {
    CardDef::from_faces(CardDef::catalog(faces()).get(name).unwrap_or_else(|| panic!("no Oracle card {name}")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A card by its Oracle name, checked to be one the engine runs.
pub fn oracle(name: &str) -> CardDef {
    let def = CardDef::from_faces(CardDef::catalog(faces()).get(name).unwrap_or_else(|| panic!("no Oracle card {name}")))
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    game::play::check(&def).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    def
}

/// A board: who has which cards where, by their Oracle names (real cards only).
/// Libraries contain only explicitly declared cards; padding is opt-in through `fill`.
pub struct Board {
    players: usize,
    life: i32,
    defs: Vec<CardDef>,
    index: BTreeMap<String, usize>,
    puts: Vec<(usize, usize, Zone, bool)>,
    counters: Vec<(String, String, u32)>,
    commanders: Vec<(usize, String)>,
    fill: usize,
    active: usize,
    part: Part,
    labels: Vec<(String, usize)>,
}

impl Board {
    pub fn new(players: usize) -> Board {
        Board { players, life: 20, defs: Vec::new(), index: BTreeMap::new(), puts: Vec::new(), counters: Vec::new(), commanders: Vec::new(), fill: 0, active: 0, part: Part::PrecombatMain, labels: Vec::new() }
    }
    pub fn life(mut self, life: i32) -> Board { self.life = life; self }
    /// Explicitly add n Wastes below the declared library cards of each player (default: 0).
    pub fn fill(mut self, n: usize) -> Board { self.fill = n; self }
    /// Set the active player and initial priority holder, without advancing the game.
    pub fn active(mut self, p: usize) -> Self { assert!(p < self.players); self.active = p; self }
    /// Select the initial phase/step without executing its turn-based actions.
    pub fn at(mut self, part: Part) -> Self { self.part = part; self }
    /// Insert one explicitly labeled incarnation; duplicates without labels remain legal.
    pub fn named(mut self, p: usize, zone: Zone, label: &str, name: &str) -> Self {
        assert!(!self.labels.iter().any(|(l, _)| l == label), "duplicate fixture label {label}");
        let index = self.puts.len();
        self = self.put(p, &[name], zone, false);
        self.labels.push((label.into(), index));
        self
    }
    fn def(&mut self, name: &str) -> usize {
        if let Some(i) = self.index.get(name) { return *i; }
        self.defs.push(oracle(name));
        self.index.insert(name.to_string(), self.defs.len() - 1);
        self.defs.len() - 1
    }
    fn put(mut self, player: usize, names: &[&str], zone: Zone, tapped: bool) -> Board {
        for name in names { let d = self.def(name); self.puts.push((player, d, zone, tapped)); }
        self
    }
    pub fn hand(self, p: usize, names: &[&str]) -> Board { self.put(p, names, Zone::Hand, false) }
    pub fn battlefield(self, p: usize, names: &[&str]) -> Board { self.put(p, names, Zone::Battlefield, false) }
    pub fn tapped(self, p: usize, names: &[&str]) -> Board { self.put(p, names, Zone::Battlefield, true) }
    pub fn graveyard(self, p: usize, names: &[&str]) -> Board { self.put(p, names, Zone::Graveyard, false) }
    pub fn exile(self, p: usize, names: &[&str]) -> Board { self.put(p, names, Zone::Exile, false) }
    /// Top first: the first name is the top card.
    pub fn library(self, p: usize, names: &[&str]) -> Board {
        let rev: Vec<&str> = names.iter().rev().copied().collect();
        self.put(p, &rev, Zone::Library, false)
    }
    /// Exact initial counters on the uniquely named or labeled permanent; no event occurs.
    pub fn counters(mut self, name: &str, kind: &str, n: u32) -> Board { self.counters.push((name.into(), kind.into(), n)); self }
    /// A designated commander of that player, in the command zone (903.6).
    pub fn commander(mut self, p: usize, name: &str) -> Board {
        let d = self.def(name);
        self.puts.push((p, d, Zone::Command, false));
        self.commanders.push((p, name.into()));
        self
    }
    /// Build the declared position, without entry effects, events or state-based actions.
    pub fn build(mut self) -> T {
        let filler = (self.fill > 0).then(|| self.def("Wastes"));
        let mut g = Game::new(self.defs, self.players, self.life);
        if let Some(filler) = filler {
            for p in 0..self.players { for _ in 0..self.fill { g.put(p, filler, Zone::Library); } }
        }
        g.active = self.active;
        g.priority = self.active;
        g.part = self.part;
        let mut labels = BTreeMap::new();
        for (index, (p, d, zone, tapped)) in self.puts.into_iter().enumerate() {
            if zone == Zone::Battlefield {
                assert!(!g.defs[d].abilities().iter().any(|a| matches!(game::replace::as_enters(a), Some(game::replace::AsEnters::Value(_)))),
                    "{} needs an established entry choice: use an explicit gameplay prefix", g.defs[d].name());
            }
            let o = g.put(p, d, zone);
            if zone == Zone::Battlefield { g.set_tapped(o, tapped); }
            for (label, _) in self.labels.iter().filter(|(_, i)| *i == index) { labels.insert(label.clone(), o); }
        }
        let mut t = T { g, labels, expected_losses: Vec::new() };
        for label in t.labels.keys() { t.check_label_name(label); }
        for (p, name) in self.commanders {
            let o = t.find_in(p, Zone::Command, &name);
            t.g.designate_commander(o).unwrap();
        }
        for (name, kind, n) in self.counters { let o = t.find_in_battlefield(&name); t.g.set_initial_counters(o, &kind, n); }
        t
    }
}

/// What an action names: a card by name, or a player.
pub enum Tgt<'a> { Card(&'a str), Object(ObjId), In(usize, Zone, &'a str), Player(usize) }
impl<'a> From<&'a str> for Tgt<'a> { fn from(s: &'a str) -> Self { Tgt::Card(s) } }
pub fn on(p: usize, zone: Zone, name: &str) -> Tgt<'_> { Tgt::In(p, zone, name) }
pub fn player(p: usize) -> Tgt<'static> { Tgt::Player(p) }

/// The game under test, driven by names.
#[derive(Clone)]
pub struct T { pub g: Game, labels: BTreeMap<String, ObjId>, expected_losses: Vec<usize> }

/// Exactly where bounded resolution stopped. Ended(None) is a drawn game.
#[derive(Debug)]
pub enum Boundary { Question(usize, Asked), Priority(usize), Ended(Option<usize>) }


impl T {
    pub fn new(g: Game) -> Self { Self { g, labels: BTreeMap::new(), expected_losses: Vec::new() } }
    /// Declare allowed eliminations before running a loss scenario. Expect::check verifies all.
    pub fn expect_losses(&mut self, seats: &[usize]) -> &mut Self {
        assert!(seats.iter().all(|p| *p < self.g.players.len()));
        self.expected_losses = seats.to_vec(); self.expected_losses.sort(); self.expected_losses.dedup(); self
    }
    fn losses(&self) -> Vec<usize> { self.g.players.iter().enumerate().filter_map(|(p, s)| s.lost.then_some(p)).collect() }
    fn check_unexpected_losses(&self) {
        let lost = self.losses();
        assert!(lost.iter().all(|p| self.expected_losses.contains(p)), "unexpected eliminated players {lost:?}; allowed {:?}", self.expected_losses);
    }
    /// Bind an explicit current object. A label never follows a zone change automatically.
    pub fn bind(&mut self, label: &str, id: ObjId) -> &mut Self {
        self.live(id);
        self.check_label_name(label);
        assert!(!self.labels.contains_key(label), "duplicate label {label}");
        self.labels.insert(label.into(), id); self
    }
    fn live(&self, id: ObjId) -> ObjId {
        let o = self.g.get_object(id).unwrap_or_else(|| panic!("retired object {id:?}"));
        assert!(o.zone != Zone::Gone && self.zone_list(if matches!(o.zone, Zone::Battlefield | Zone::PhasedOut) { self.g.controller(id) } else { o.owner }, o.zone).contains(&id),
            "stale incarnation {id:?} ({}) has departed; bind its new object explicitly", o.chars.name);
        id
    }
    fn check_label_name(&self, label: &str) {
        assert!(!self.g.defs.iter().any(|d| d.name() == label), "label {label} shadows a card name; choose a distinct label");
        for p in 0..self.g.players.len() {
            for zone in [Zone::Hand,Zone::Library,Zone::Graveyard,Zone::Battlefield,Zone::Exile,Zone::Command,Zone::Stack] {
                assert!(!self.zone_list(p,zone).iter().any(|o| self.name(*o) == label), "label {label} shadows a current object name");
            }
        }
    }
    pub fn id(&self, label: &str) -> ObjId { self.check_label_name(label); self.live(*self.labels.get(label).unwrap_or_else(|| panic!("unknown label {label}"))) }
    fn unique(&self, found: Vec<ObjId>, description: &str) -> ObjId {
        assert_eq!(found.len(), 1, "expected exactly one {description}; candidates {found:?}"); found[0]
    }
    pub fn mark(&self) -> game::event::Seq { self.g.log.end() }
    /// Token creation does not have an old incarnation or a ZoneChanged event.
    pub fn created_since(&self, mark: game::event::Seq) -> Vec<ObjId> {
        self.g.log.since(mark).iter().filter_map(|e| match e {
            game::event::Event::Created { token } => Some(*token), _ => None,
        }).collect()
    }
    /// A bounded result query, never a whole-history identity tracker. Pruned marks fail.
    pub fn entered_since(&self, mark: game::event::Seq, p: usize, zone: Zone, name: &str) -> ObjId {
        let candidates = self.g.log.since(mark).iter().filter_map(|e| match e {
            game::event::Event::ZoneChanged { now: Some(id), to, .. } if *to == zone => Some(*id), _ => None,
        }).filter(|id| self.zone_list(p, zone).contains(id) && self.name(*id) == name).collect();
        self.unique(candidates, &format!("{name} entering player {p}'s {zone:?} since {mark:?}"))
    }
    fn name(&self, o: ObjId) -> String { self.g.obj(o).chars.name.clone() }
    fn zone_list(&self, p: usize, zone: Zone) -> Vec<ObjId> {
        let pl = &self.g.players[p];
        match zone {
            Zone::Hand => pl.hand.clone(), Zone::Library => pl.library.clone(), Zone::Graveyard => pl.graveyard.clone(),
            Zone::Battlefield => self.g.battlefield.iter().copied().filter(|o| self.g.controller(*o) == p).collect(),
            Zone::Exile => self.g.exile.iter().copied().filter(|o| self.g.obj(*o).owner == p).collect(),
            Zone::Command => self.g.command.iter().copied().filter(|o| self.g.obj(*o).owner == p).collect(),
            Zone::Stack => self.g.stack.iter().copied().filter(|o| self.g.controller(*o) == p).collect(),
            Zone::PhasedOut => self.g.phased_out.iter().copied().filter(|o| self.g.controller(*o) == p).collect(),
            Zone::Gone => Vec::new(),
        }
    }
    /// The one object of that name in that player's zone. Battlefield and stack use controller;
    /// exile and command use owner.
    pub fn find_in(&self, p: usize, zone: Zone, name: &str) -> ObjId {
        if self.labels.contains_key(name) {
            let id = self.id(name);
            assert!(self.zone_list(p, zone).contains(&id), "label {name} is not in player {p}'s {zone:?}"); return id;
        }
        let found = self.zone_list(p, zone).into_iter().filter(|o| self.name(*o) == name).collect();
        self.unique(found, &format!("{name} in player {p}'s {zone:?}"))
    }
    pub fn find_in_battlefield(&self, name: &str) -> ObjId {
        if self.labels.contains_key(name) { let id = self.id(name); assert!(self.g.battlefield.contains(&id), "{name} is not on the battlefield"); return id; }
        self.unique(self.g.battlefield.iter().copied().filter(|o| self.name(*o) == name).collect(), &format!("{name} on the battlefield"))
    }
    /// A label or globally unique name. Prefer scoped lookup for ordinary names.
    pub fn find(&self, name: &str) -> ObjId {
        if self.labels.contains_key(name) { return self.id(name); }
        let mut all: Vec<ObjId> = self.g.stack.iter().chain(&self.g.battlefield).copied().collect();
        for p in 0..self.g.players.len() { for z in [Zone::Hand, Zone::Graveyard, Zone::Exile, Zone::Command, Zone::Library] { all.extend(self.zone_list(p, z)); } }
        self.unique(all.into_iter().filter(|o| self.name(*o) == name).collect(), name)
    }
    fn target(&self, t: Tgt) -> Target { match t { Tgt::Card(n) => Target::Object(self.find(n)), Tgt::Object(id) => Target::Object(self.live(id)), Tgt::In(p,z,n) => Target::Object(self.find_in(p,z,n)), Tgt::Player(p) => Target::Player(p) } }

    /// Add mana to a pool: "{1}{U}{U}", generic as colorless.
    pub fn mana(&mut self, p: usize, cost: &str) -> &mut T {
        use ir::{Color, mana::ManaSymbol};
        for part in cost.split('{').filter(|s| !s.is_empty()).map(|s| s.trim_end_matches('}')) {
            let symbols: Vec<ManaSymbol> = match part {
                "W" => vec![ManaSymbol::Color(Color::White)], "U" => vec![ManaSymbol::Color(Color::Blue)],
                "B" => vec![ManaSymbol::Color(Color::Black)], "R" => vec![ManaSymbol::Color(Color::Red)],
                "G" => vec![ManaSymbol::Color(Color::Green)], "C" => vec![ManaSymbol::Colorless],
                n => vec![ManaSymbol::Colorless; n.parse::<usize>().unwrap_or_else(|_| panic!("mana {n}"))],
            };
            for s in symbols { self.g.add_mana(p, s); }
        }
        self
    }
    /// Cast a card from that player's hand, with one target per group.
    pub fn cast<'a>(&mut self, p: usize, name: &str, targets: Vec<Tgt<'a>>, with: game::state::Pool) -> Result<&mut T, String> {
        let card = self.find_in(p, Zone::Hand, name);
        let targets = targets.into_iter().map(|t| vec![self.target(t)]).collect();
        self.g.act(p, Move::Paying(with,Box::new(Move::Cast { card, targets })))?;
        self.check_unexpected_losses();
        Ok(self)
    }
    /// Activate the specified printed ability index; no search or payment planning.
    pub fn activate<'a>(&mut self, p: usize, name: &str, ability: usize, targets: Vec<Tgt<'a>>, with: game::state::Pool) -> Result<&mut T, String> {
        let source = self.find_in(p, Zone::Battlefield, name);
        let targets = targets.into_iter().map(|t| vec![self.target(t)]).collect();
        self.g.act(p, Move::Paying(with,Box::new(Move::Activate { source, ability, targets, paying: vec![] })))?;
        self.check_unexpected_losses();
        Ok(self)
    }
    /// Every living player passes once. A new question is returned by asked(), never answered.
    pub fn pass_all(&mut self) -> &mut T {
        let count = self.g.players.iter().filter(|p| !p.lost).count();
        for _ in 0..count {
            self.check_unexpected_losses();
            let Some((p, Asked::Priority)) = self.g.asked() else { panic!("expected priority, got {:?}", self.g.asked()) };
            self.g.act(p, Move::Pass).unwrap();
        }
        self.check_unexpected_losses(); self
    }
    /// Pass priority only, until the next question, empty-stack priority, or game end.
    pub fn settle(&mut self) -> Boundary {
        for _ in 0..2000 {
            self.check_unexpected_losses();
            match self.g.asked() {
                None => return Boundary::Ended(self.g.winner()),
                Some((p, Asked::Priority)) if self.g.stack.is_empty() => return Boundary::Priority(p),
                Some((p, Asked::Priority)) => self.g.act(p, Move::Pass).unwrap(),
                Some((p, asked)) => return Boundary::Question(p, asked),
            }
        }
        panic!("resolution failed to reach a boundary in 2000 passes");
    }
    /// Resolve without choices; unexpected decisions are failures, even with an empty stack.
    pub fn resolve(&mut self) -> &mut T {
        let boundary = self.settle();
        assert!(matches!(boundary, Boundary::Priority(_)), "unexpected resolution boundary {boundary:?}"); self
    }
    pub fn asked(&mut self) -> Option<(usize, Asked)> { self.g.asked() }
    pub fn answer(&mut self, p: usize, answer: Answer) -> Result<&mut T, String> { self.g.act(p, Move::Choose(answer))?; self.check_unexpected_losses(); Ok(self) }
    /// Answer an object question with these cards, by name, in this order.
    pub fn choose(&mut self, p: usize, names: &[&str]) -> Result<&mut T, String> {
        let objs = names.iter().map(|n| self.find(n)).collect();
        self.answer(p, Answer::Objects(objs))
    }
    /// Explicit opt-in no-combat policy while advancing to a phase; all other questions fail.
    pub fn to_no_combat(&mut self, p: usize, part: Part) -> &mut T {
        for _ in 0..2000 {
            self.check_unexpected_losses();
            assert!(!self.g.finished(), "game ended before requested phase");
            if self.g.active == p && self.g.part == part && self.g.stack.is_empty() && matches!(self.g.asked(), Some((_, Asked::Priority))) { return self; }
            match self.g.decision.clone() {
                Some(Decision::Attackers { player }) => { self.g.act(player, Move::Attack(vec![])).unwrap(); }
                Some(Decision::Blockers { player }) => { self.g.act(player, Move::Block(vec![])).unwrap(); }
                _ => { self.g.act(self.g.decision_maker(self.g.priority), Move::Pass).unwrap(); }
            }
        }
        panic!("never reached player {p}'s {part:?}");
    }

    /// A diagnostic projection by names, not the whole game or its ordered hidden zones.
    pub fn state(&self) -> State {
        let names = |v: Vec<ObjId>| { let mut n: Vec<String> = v.into_iter().map(|o| self.name(o)).collect(); n.sort(); n };
        State { players: (0..self.g.players.len()).map(|p| PlayerState {
            life: self.g.players[p].life,
            hand: names(self.zone_list(p, Zone::Hand)),
            library: self.g.players[p].library.len(),
            graveyard: names(self.zone_list(p, Zone::Graveyard)),
            exile: names(self.zone_list(p, Zone::Exile)),
            battlefield: { let mut b: Vec<Perm> = self.zone_list(p, Zone::Battlefield).into_iter().map(|o| Perm {
                name: self.name(o), tapped: self.g.obj(o).tapped,
                counters: self.g.obj(o).counters.iter().filter(|(_, n)| **n > 0).map(|(k, n)| (k.clone(), *n)).collect(),
                pt: self.g.power(o).zip(self.g.toughness(o)),
            }).collect(); b.sort_by(|a, b| a.name.cmp(&b.name)); b },
        }).collect(), stack: self.g.stack.iter().map(|o| self.name(*o)).collect() }
    }
    /// Properties of the next state, all checked and reported together.
    pub fn expect(&self) -> Expect<'_> { Expect { t: self, errors: Vec::new(), rules: Vec::new(), steps: Vec::new() } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Perm { pub name: String, pub tapped: bool, pub counters: BTreeMap<String, u32>, pub pt: Option<(i32, i32)> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState { pub life: i32, pub hand: Vec<String>, pub library: usize, pub graveyard: Vec<String>, pub exile: Vec<String>, pub battlefield: Vec<Perm> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State { pub players: Vec<PlayerState>, pub stack: Vec<String> }

#[must_use = "call .check() to assert the collected expectations"]
pub struct Expect<'a> {
    t: &'a T,
    errors: Vec<String>,
    /// The rules the steps after `.rules(…)` give evidence of, until the next `.rules(…)`.
    rules: Vec<String>,
    /// Each step, where it is written, the rules it cites (none: uncited) and whether it held.
    steps: Vec<(&'static std::panic::Location<'static>, Vec<String>, bool)>,
}
impl<'a> Expect<'a> {
    /// The following steps are evidence of these rules together (see `support::cite`); steps
    /// before any `.rules(…)` are uncited.
    pub fn rules<I: IntoIterator<Item = S>, S: Into<String>>(mut self, ids: I) -> Self { self.rules = cite::ids(ids); self }
    #[track_caller]
    fn fail(mut self, ok: bool, msg: String) -> Self {
        if !ok {
            let cited = if self.rules.is_empty() { String::new() } else { format!(" [rules {}]", self.rules.join(", ")) };
            self.errors.push(msg + &cited);
        }
        self.steps.push((std::panic::Location::caller(), self.rules.clone(), ok));
        self
    }
    #[track_caller]
    pub fn life(self, p: usize, life: i32) -> Self { let got = self.t.g.players[p].life; self.fail(got == life, format!("player {p} life {got}, expected {life}")) }
    /// Exactly these cards (any order) in that player's zone; stack/battlefield use controller.
    #[track_caller]
    pub fn zone_exact(self, p: usize, zone: Zone, names: &[&str]) -> Self {
        let mut got: Vec<String> = self.t.zone_list(p, zone).into_iter().map(|o| self.t.name(o)).collect();
        got.sort();
        let mut want: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        want.sort();
        let ok = got == want;
        self.fail(ok, format!("player {p} {zone:?} is {got:?}, expected {want:?}"))
    }
    /// Compatibility for existing explicit multiset assertions.
    #[track_caller]
    pub fn zone(self, p: usize, zone: Zone, names: &[&str]) -> Self { self.zone_exact(p, zone, names) }
    /// Library top first; graveyard oldest first; stack bottom first (engine storage order).
    #[track_caller]
    pub fn zone_ordered(self, p: usize, zone: Zone, names: &[&str]) -> Self {
        let mut ids = self.t.zone_list(p, zone);
        if zone == Zone::Library { ids.reverse(); }
        let got: Vec<_> = ids.into_iter().map(|o| self.t.name(o)).collect();
        let want: Vec<_> = names.iter().map(|n| n.to_string()).collect();
        self.fail(got == want, format!("player {p} {zone:?} order {got:?}, expected {want:?}"))
    }
    #[track_caller]
    pub fn hidden_from(self, p: usize, labels: &[&str]) -> Self {
        let view = game::snapshot::Snapshot::of(&self.t.g, Some(p));
        let ids: Vec<_> = labels.iter().map(|l| self.t.id(l)).collect();
        let visible: Vec<_> = view.objects.iter().filter(|o| ids.contains(&o.id)).map(|o| o.id).collect();
        self.fail(visible.is_empty(), format!("player {p} unexpectedly sees {visible:?}"))
    }
    /// Non-consuming observation assertion, grouped exactly as the engine delivered them.
    #[track_caller]
    pub fn observations(self, p: usize, expected: &[&[&str]]) -> Self {
        let got: Vec<Vec<_>> = self.t.g.observations(p).iter().map(|o| o.cards.iter().map(|c| c.name.as_str()).collect()).collect();
        let ok = got.iter().map(Vec::as_slice).eq(expected.iter().copied());
        let message = format!("player {p} observations {got:?}, expected {expected:?}");
        self.fail(ok, message)
    }
    #[track_caller]
    pub fn contains(self, p: usize, zone: Zone, name: &str) -> Self {
        let ok = self.t.zone_list(p, zone).iter().any(|o| self.t.name(*o) == name);
        self.fail(ok, format!("no {name} in player {p}'s {zone:?}"))
    }
    #[track_caller]
    pub fn tapped(self, name: &str, tapped: bool) -> Self { let o = self.t.find_in_battlefield(name); let got = self.t.g.obj(o).tapped; self.fail(got == tapped, format!("{name} tapped {got}, expected {tapped}")) }
    #[track_caller]
    pub fn pt(self, name: &str, p: i32, t: i32) -> Self {
        let o = self.t.find_in_battlefield(name);
        let got = self.t.g.power(o).zip(self.t.g.toughness(o));
        self.fail(got == Some((p, t)), format!("{name} is {got:?}, expected {p}/{t}"))
    }
    #[track_caller]
    pub fn counters(self, name: &str, kind: &str, n: u32) -> Self {
        let o = self.t.find_in_battlefield(name);
        let got = self.t.g.obj(o).counters.get(kind).copied().unwrap_or(0);
        self.fail(got == n, format!("{name} has {got} {kind}, expected {n}"))
    }
    #[track_caller]
    pub fn stack(self, names: &[&str]) -> Self {
        let got: Vec<String> = self.t.g.stack.iter().map(|o| self.t.name(*o)).collect();
        self.fail(got == names, format!("stack {got:?}, expected {names:?}"))
    }
    #[track_caller]
    pub fn holds(self, ok: impl FnOnce(&Game) -> bool, what: &str) -> Self { let ok = ok(&self.t.g); self.fail(ok, what.to_string()) }
    #[track_caller]
    pub fn check(self) {
        assert_eq!(self.t.losses(), self.t.expected_losses, "unexpected eliminated players");
        // each cited step's outcome; the steps are checked together, so all are recorded
        for (at, rules, ok) in &self.steps {
            if !rules.is_empty() { cite::record(rules, None, at, if *ok { "passed" } else { "failed" }); }
        }
        assert!(self.errors.is_empty(), "\n{}\nstate: {:#?}", self.errors.join("\n"), self.t.state());
    }
}
