//! Card definitions: a face's characteristics and its abilities as the grammar reads them.

use grammar::ability::abilities;
use grammar::kernel::Outcome;
use ir::mana::{ManaCost, ManaSymbol};
use ir::{Ability, CardType, Color, Mode, Subtype, Supertype};
use oracle::classify::{Kind as Row, classify};
use oracle::normalize::normalize;
use oracle::token::{Kind, detokenize, tokenize};

/// What a card prints: all that a definition is compiled from.
#[derive(Clone)]
struct Printed {
    halves: Vec<CardDef>,
    name: String,
    cost: Option<ManaCost>,
    supertypes: Vec<Supertype>,
    types: Vec<CardType>,
    subtypes: Vec<Subtype>,
    colors: Vec<Color>,
    text_colors: Vec<Color>,
    pt: Option<(i32, i32)>,
    loyalty: Option<u32>,
    abilities: Vec<Ability>,
    modes: Vec<Vec<Mode>>,
}

/// One physical card, shared read-only by every game; a split card contains both halves.
/// What it prints and what was compiled from that are one value: it is made whole by
/// `compile` and read, never changed, so the compiled parts cannot go stale. A different
/// card is another definition, built from its own printed parts.
///
/// ```compile_fail,E0616
/// let mut def = game::CardDef::new("Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
/// def.abilities.clear();
/// ```
/// ```compile_fail,E0616
/// let mut def = game::CardDef::new("Bears", "{1}{G}", "Creature — Bear", "", Some((2, 2))).unwrap();
/// def.name = String::from("Forest");
/// ```
#[derive(Debug, Clone)]
pub struct CardDef {
    pub(crate) companion: Option<crate::Res<crate::companion::Condition>>,
    pub(crate) cast_permissions: Vec<crate::cast_permission::Registration>,
    pub(crate) library_permissions: Vec<crate::library_top::Registration>,
    pub(crate) opening: Vec<usize>,
    pub(crate) admission: crate::Res<()>,
    pub(crate) costs: crate::cost::Definitions,
    pub(crate) plot: Option<crate::Res<crate::cost::Plan>>,
    /// A split card is one card with two castable halves (709.2). This definition holds the
    /// combined characteristics used outside the stack; the halves keep their own text.
    pub(crate) halves: Vec<CardDef>,
    /// The second face of a double-faced card (712.2–3). This definition is the front,
    /// which applies outside the stack/battlefield (712.8a). Modal backs retain their own
    /// mana cost/value; transforming backs use the front's value.
    pub(crate) back: Option<Box<CardDef>>,
    /// Modal faces are chosen at casting/land play, not joined or transformed to play them.
    pub(crate) modal: bool,
    /// Alternative spell characteristics (715.2, 720.2); normal characteristics apply elsewhere.
    pub(crate) alternative_spell: Option<Box<CardDef>>,
    /// Copiable inset characteristics, which the physical card cannot cast (722.2b, 722.3).
    pub(crate) prepare_spell: Option<Box<CardDef>>,
    /// This definition describes the generated spell, including its copies (722.3d).
    pub(crate) is_prepare_spell: bool,
    /// Of the back face of a double-faced card: the mana value of its front face, which is
    /// its own (202.3b).
    pub(crate) front_mana_value: Option<u32>,
    pub(crate) name: String,
    /// None: no mana cost (a land), which is not a cost of {0} (202.1b).
    pub(crate) cost: Option<ManaCost>,
    pub(crate) supertypes: Vec<Supertype>,
    pub(crate) types: Vec<CardType>,
    pub(crate) subtypes: Vec<Subtype>,
    /// From the mana cost (202.2), overridden by a color indicator where present (204.2).
    pub(crate) colors: Vec<Color>,
    /// Mana-symbol colors in non-reminder rules text (903.4); immutable printed metadata.
    text_colors: Vec<Color>,
    pub(crate) pt: Option<(i32, i32)>,
    /// The loyalty a planeswalker card is printed with (306.5a): the loyalty counters it
    /// enters with (306.5b).
    pub(crate) loyalty: Option<u32>,
    pub(crate) abilities: Vec<Ability>,
    /// One declaration per printed keyword instance, cached before any keyword consumer.
    pub(crate) keyword_declarations: Vec<crate::keyword::Instance>,
    /// Executable meanings of printed keywords, paired with the printed row they came from.
    /// Appended after printed abilities at runtime so existing ability indices remain stable.
    pub(crate) keyword_abilities: Vec<(usize, Ability)>,
    /// The Oracle text's words for each "~" and "§" of each ability, in the order of
    /// `abilities` ([`CardDef::forms`]).
    forms: Forms,
    /// Of a Room, what a permanent of it is with neither half unlocked ([`CardDef::room`]).
    room: Option<Box<CardDef>>,
    /// Of a split card's half, which half it is: 0 the left (709.1).
    door: Option<usize>,
    /// The passive keyword abilities it prints (702), lowered once: what an object it
    /// represents has before any effect gives it another (613.1f).
    pub(crate) keywords: Vec<crate::keyword::Held>,
    /// The mode rows printed under each ability, in the order of `abilities` (700.2): the
    /// options a modal ability's "Choose one —" chooses among; none for an ability that is
    /// not modal.
    pub(crate) modes: Vec<Vec<Mode>>,
    /// Owned effects compiled once when this definition is loaded. Errors are admission
    /// results, not parse failures. Clones share immutable programs across games.
    pub(crate) activation_keywords: Vec<Option<ir::Keyword>>,
    pub(crate) activation_modifiers: Vec<crate::program::registration::Registered<crate::activate::Modifier>>,
    pub(crate) unused_permissions: Vec<crate::program::registration::Registered<crate::activate::UnusedPermission>>,
    /// Copiable semantic declarations, before executable lowering.
    declaration_abilities: Vec<Ability>,
    pub(crate) programs: Vec<crate::program::CompiledEffect>,
    pub(crate) links: crate::binding::Links,
    /// Where each of its abilities functions (113.6) and what its static abilities say of
    /// casting it, read once when this definition is loaded.
    pub(crate) functioning: crate::functions::Functioning,
    /// Casting-cost increases/reductions compiled from static abilities (601.2f).
    pub(crate) cost_changes: Vec<crate::program::registration::Registered<crate::cost_change::Modifier>>,
    /// The restrictions and requirements its static abilities state, lowered once when this
    /// definition is loaded, each with the refusal of one the engine does not hold in force.
    pub(crate) restrictions: Vec<crate::program::registration::Registered<crate::program::registration::Restriction>>,
    /// What its static abilities say their subjects are (611.3), lowered once in the same
    /// way, each with the refusal of one the engine does not apply.
    pub(crate) characteristics: Vec<crate::program::registration::Registered<crate::program::registration::Characteristic>>,
    /// What its triggered abilities watch for (603.1), lowered once in the same way, each
    /// with the refusal of one the engine does not watch for.
    pub(crate) triggers: Vec<crate::program::registration::Registered<crate::trigger::Template>>,
    /// Effective additional-trigger statements (603.2d), compiled once.
    pub(crate) trigger_modifiers: Vec<crate::program::registration::Registered<crate::trigger::ModifierTemplate>>,
    /// The replacement effects its abilities are (614.1), lowered once in the same way, each
    /// with the refusal of one the engine does not apply.
    pub(crate) replacements: Vec<crate::program::registration::Registered<crate::replace::Replacement>>,
    /// What an Aura enchants (303.4a, 702.5): all its enchant abilities as one target group,
    /// lowered once in the same way, or the refusal of one the engine cannot tell. None for
    /// what is not an Aura.
    pub(crate) enchant: crate::Res<Option<crate::announcement::TargetSpec>>,
}

impl CardDef {
    /// A card from its printed parts: `CardDef::new("Shock", "{R}", "Instant", "Shock deals 2
    /// damage to any target.", None)`. Fails when the grammar does not read the text.
    pub fn new(name: &str, mana_cost: &str, type_line: &str, text: &str, pt: Option<(i32, i32)>) -> Result<CardDef, String> {
        let (supertypes, types, subtypes) = type_line_parts(type_line)?;
        let cost = mana_cost_of(mana_cost)?;
        let colors = cost.as_ref().map(colors_of).unwrap_or_default();
        let (abilities, modes, text_colors, forms, station) = read_as(text, name, name, type_line, pt, None)?;
        let pt = pt.filter(|_| !station);
        Ok(Self::compile(Printed { halves: Vec::new(), name: name.to_string(), cost, supertypes, types, subtypes, colors, text_colors, pt, loyalty: None, abilities, modes }).with_forms(forms))
    }

    /// A planeswalker card from its printed parts, with the loyalty printed on it (306.5a):
    /// `CardDef::planeswalker("Garruk Wildspeaker", "{2}{G}{G}", "Legendary Planeswalker —
    /// Garruk", "+1: Untap two target lands.", 3)`.
    pub fn planeswalker(name: &str, mana_cost: &str, type_line: &str, text: &str, loyalty: u32) -> Result<CardDef, String> {
        let (supertypes, types, subtypes) = type_line_parts(type_line)?;
        let cost = mana_cost_of(mana_cost)?;
        let colors = cost.as_ref().map(colors_of).unwrap_or_default();
        let (abilities, modes, text_colors, forms, _) = read_as(text, name, name, type_line, None, None)?;
        Ok(Self::compile(Printed { halves: Vec::new(), name: name.to_string(), cost, supertypes, types, subtypes, colors, text_colors, pt: None, loyalty: Some(loyalty), abilities, modes }).with_forms(forms))
    }

    /// The one place a definition is made: everything derived from the printed parts is
    /// compiled here, each step reading what the steps before it derived.
    /// With the Oracle text's words for each object it refers to by name, per ability.
    fn with_forms(mut self, forms: Forms) -> CardDef {
        self.forms = forms;
        self
    }

    /// How the Oracle text of the ability at `index` words each "~" and "§" it prints, in
    /// order: "this creature", "This token", the card's name. Nothing for an ability not
    /// printed as read (a keyword's meaning, a copy's).
    pub(crate) fn forms(&self, index: usize) -> &[String] {
        self.forms.abilities.get(index).map_or(&[], |f| f.as_slice())
    }

    /// As [`CardDef::forms`], of each mode row printed under the ability at `index`.
    pub(crate) fn mode_forms(&self, index: usize) -> &[Vec<String>] {
        self.forms.modes.get(index).map_or(&[], |f| f.as_slice())
    }

    fn compile(printed: Printed) -> CardDef {
        // the predefined tokens' definitions, read before any card that creates one
        crate::token::ready();
        let Printed { halves, name, cost, supertypes, types, subtypes, colors, text_colors, pt, loyalty, mut abilities, modes } = printed;
        // "Max speed — [Ability]" is the static ability it means (702.178a)
        for a in &mut abilities { if let Some(meant) = crate::speed::max_speed(a) { *a = meant; } }
        let declaration_abilities = abilities.clone();
        for a in &mut abilities { if let Some(meant) = crate::case::to_solve(a) { *a = meant; } }
        let activation_keywords = abilities.iter_mut().map(|ability| {
            crate::activate::keyword_activation(ability).map(|(keyword, lowered)| {
                *ability = lowered;
                keyword
            })
        }).collect();
        let keyword_declarations = crate::keyword::declarations(&abilities);
        let keyword_abilities = crate::keyword::executable_in(&keyword_declarations);
        let opening = crate::start::compile(&abilities);
        let mut def = CardDef { activation_modifiers: Vec::new(), unused_permissions: Vec::new(), declaration_abilities, activation_keywords, companion: None, cast_permissions: Vec::new(), library_permissions: Vec::new(), opening, back: None, modal: false, alternative_spell: None, prepare_spell: None, is_prepare_spell: false, front_mana_value: None, admission: Ok(()), costs: Default::default(), plot: None, keyword_declarations, keyword_abilities, keywords: Vec::new(), halves, name, cost, supertypes, types, subtypes, colors, text_colors, pt, loyalty, abilities, modes, programs: Vec::new(), links: crate::binding::Links::default(), functioning: Default::default(), cost_changes: Vec::new(), restrictions: Vec::new(), characteristics: Vec::new(), triggers: Vec::new(), trigger_modifiers: Vec::new(), replacements: Vec::new(), enchant: Ok(None), forms: Forms::default(), room: None, door: None };
        def.functioning = crate::functions::Functioning::compile(&def);
        def.links = crate::binding::compile(&def);
        def.keywords = crate::keyword::held(&def);
        def.enchant = crate::attach::compile(&def);
        def.costs = crate::cost::Definitions::compile(&def);
        def.plot = crate::plot::compile(&def);
        def.cast_permissions = crate::cast_permission::registrations(&def);
        def.library_permissions = crate::library_top::registrations(&def);
        def.companion = crate::companion::compile(&def);
        def.programs = crate::program::compile_definition(&def);
        def.cost_changes = crate::cost_change::registrations(&def);
        def.unused_permissions = crate::activate::unused_permissions(&def);
        def.activation_modifiers = crate::activate::modifiers(&def);
        def.restrictions = crate::program::registration::restrictions(&def);
        def.characteristics = crate::program::registration::characteristics(&def);
        def.triggers = crate::trigger::templates(&def);
        def.trigger_modifiers = crate::trigger::modifiers(&def);
        def.replacements = crate::replace::replacements(&def);
        def.admission = crate::play::validate_definition(&def);
        def
    }

    pub fn name(&self) -> &str { &self.name }
    /// None: no mana cost (a land), which is not a cost of {0} (202.1b).
    pub fn cost(&self) -> Option<&ManaCost> { self.cost.as_ref() }
    pub fn supertypes(&self) -> &[Supertype] { &self.supertypes }
    pub fn types(&self) -> &[CardType] { &self.types }
    pub fn subtypes(&self) -> &[Subtype] { &self.subtypes }
    pub fn colors(&self) -> &[Color] { &self.colors }
    /// Printed color identity for deck construction (903.4), including alternative faces.
    /// This is not the object's effective color, and never queries a game or copy effect.
    pub fn color_identity(&self) -> Vec<Color> {
        let mut colors = self.cost.as_ref().map(colors_of).unwrap_or_default();
        colors.extend(&self.colors); // color indicator, or the cost-derived baseline
        colors.extend(&self.text_colors);
        for characteristic in &self.characteristics {
            if let Ok(c) = &characteristic.result { colors.extend(c.defined_colors()); }
        }
        for other in self.halves.iter().chain(self.back.as_deref()).chain(self.alternative_spell.as_deref()).chain(self.prepare_spell.as_deref()) {
            colors.extend(other.color_identity());
        }
        [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green]
            .into_iter().filter(|c| colors.contains(c)).collect()
    }

    pub fn pt(&self) -> Option<(i32, i32)> { self.pt }
    pub fn abilities(&self) -> &[Ability] { &self.abilities }
    /// The mode rows printed under each ability, in the order of `abilities`.
    pub fn modes(&self) -> &[Vec<Mode>] { &self.modes }
    /// The two halves of a split card; none for any other.
    pub fn halves(&self) -> &[CardDef] { &self.halves }

    /// A permanent split card, one whose halves share a type line (709.5): a Room.
    pub fn is_room(&self) -> bool { self.halves.first().is_some_and(|h| !h.is(CardType::Instant) && !h.is(CardType::Sorcery)) }

    /// What a permanent of this card is with these halves unlocked (709.5): a locked half has
    /// no name, mana cost or rules text, so with one unlocked it is that half, with both the
    /// whole card, and with neither only its shared type line. Any other card is itself.
    pub fn room(&self, unlocked: [bool; 2]) -> &CardDef {
        match (&self.room, unlocked) {
            (None, _) | (Some(_), [true, true]) => self,
            (Some(locked), [false, false]) => locked,
            (Some(_), [left, _]) => &self.halves[usize::from(!left)],
        }
    }

    /// The half of a split card the ability at `index` of this definition is printed on:
    /// "this door" of a Room (709.5j). None for a card that is not split.
    pub(crate) fn door_of(&self, index: usize) -> Option<usize> {
        match self.halves.first() {
            Some(left) => Some(usize::from(index >= left.abilities.len())),
            None => self.door,
        }
    }

    /// Names accepted by deck and setup readers name whole physical cards, never a
    /// detached back/flipped face (710.2, 712.8a). Split/adventure aliases retain all faces.
    /// Unsupported layouts still reach `from_faces` whole, so setup cannot silently run
    /// just the front. Face parsing and in-game name choices have their own vocabulary.
    pub fn catalog(faces: &[oracle::corpus::Face]) -> std::collections::BTreeMap<&str, Vec<&oracle::corpus::Face>> {
        let mut cards: std::collections::BTreeMap<&str, Vec<_>> = std::collections::BTreeMap::new();
        for face in faces { cards.entry(&face.oracle_id).or_default().push(face); }
        let mut names = std::collections::BTreeMap::new();
        for card in cards.values_mut() {
            card.sort_by_key(|f| f.face);
            let front = card[0];
            // A melded permanent's combined back is not a card (712.4b). The corpus
            // stores it separately, with no mana cost, unlike its two front cards.
            let melded = front.layout == "meld" && front.mana_cost.as_deref().unwrap_or("").is_empty()
                && !front.type_line.contains("Land");
            if front.face != 0 || melded { continue; }
            names.insert(front.face_name.as_str(), card.clone());
            names.insert(front.name.as_str(), card.clone());
        }
        for card in cards.values() {
            if !matches!(card[0].layout.as_str(), "split" | "adventure" | "prepare") { continue; }
            for face in card {
                // A genuine standalone card takes precedence over an alternative-face
                // alias with the same name; the full name always disambiguates.
                names.entry(face.face_name.as_str()).or_insert_with(|| card.clone());
            }
        }
        names
    }

    pub fn from_face(f: &oracle::corpus::Face) -> Result<CardDef, String> {
        if f.layout == "prepare" { return Err("a preparation card requires normal and inset characteristics (722.2)".into()); }
        if f.layout == "split" { return Err("a split card requires both halves (709.2)".into()); }
        if f.layout == "adventure" { return Err("an adventurer card requires both sets of characteristics (715.2)".into()); }
        Self::read_face(f)
    }

    fn read_face(f: &oracle::corpus::Face) -> Result<CardDef, String> {
        let (abilities, modes, text_colors, forms, station) = read_as(&f.oracle_text, &f.name, &f.face_name, &f.type_line, boxed_pt(f), None)?;
        let pt = match (&f.power, &f.toughness) {
            // it has them only from its station symbol, on the battlefield (721.2b, 721.2c)
            _ if station => None,
            (Some(p), Some(t)) => Some((
                printed_number(p, true, &abilities)?, printed_number(t, false, &abilities)?,
            )),
            _ => None,
        };
        let (supertypes, types, subtypes) = type_line_parts(&f.type_line)?;
        let cost = mana_cost_of(f.mana_cost.as_deref().unwrap_or(""))?;
        let mut colors = cost.as_ref().map(colors_of).unwrap_or_default();
        if let Some(indicator) = &f.color_indicator { colors = indicated_colors(indicator)?; }
        // "X" and "*" are not a number of counters: such a card stays without one and is refused
        let loyalty = f.loyalty.as_deref().and_then(|l| l.parse().ok());
        Ok(Self::compile(Printed { halves: Vec::new(), name: f.face_name.clone(), cost, supertypes, types, subtypes, colors, text_colors, pt, loyalty, abilities, modes }).with_forms(forms))
    }

    /// Everything that keeps the grammar from reading one face, in the order
    /// [`CardDef::from_face`] meets it: the first's reason is the error reading gives. Each
    /// printed row is read on its own, so each row the grammar declines or reads two ways is
    /// kept, with where it stopped; and each printed characteristic it cannot read. Empty when
    /// the face is read. What only the read text could tell (a "*" that a characteristic-defining
    /// ability defines, 208.2a) is left unanalyzed, and said to be.
    pub fn unread_face(f: &oracle::corpus::Face) -> Vec<crate::play::Blocker> {
        let mut all = Vec::new();
        let text = read_as(&f.oracle_text, &f.name, &f.face_name, &f.type_line, boxed_pt(f), Some(&mut all));
        if let (Some(p), Some(t)) = (&f.power, &f.toughness) {
            for (printed, power) in [(p, true), (t, false)] {
                match &text {
                    // it has them only from its station symbol, on the battlefield (721.2b, 721.2c)
                    Ok((.., true)) => {}
                    Ok((abilities, ..)) => if let Err(e) = printed_number(printed, power, abilities) { all.push(crate::play::Blocker::parse(e)); },
                    Err(_) => if printed_number(printed, power, &[]).is_err() && let Some(first) = all.first_mut() {
                        first.unanalyzed.push(format!("the printed {} {printed:?}: read with the abilities of the text (208.2a, 721.2b)", if power { "power" } else { "toughness" }));
                    },
                }
            }
        }
        if let Err(e) = type_line_parts(&f.type_line) { all.push(crate::play::Blocker::parse(e)); }
        if let Err(e) = mana_cost_of(f.mana_cost.as_deref().unwrap_or("")) { all.push(crate::play::Blocker::parse(e)); }
        if let Some(Err(e)) = f.color_indicator.as_deref().map(indicated_colors) { all.push(crate::play::Blocker::parse(e)); }
        all
    }

/// Read one whole card. Split halves are ordered by their printed position, not by name.
    pub fn from_faces(faces: &[&oracle::corpus::Face]) -> Result<CardDef, String> {
        if faces.len() == 1 && faces[0].layout == "modal_dfc" { return Err("a modal double-faced card requires both faces (712.3)".into()); }
        if faces.len() == 1 { return Self::from_face(faces[0]); }
        if let [a, b] = faces && a.layout == "prepare" && b.layout == "prepare" && a.oracle_id == b.oracle_id {
            let (normal, spell) = if a.face <= b.face { (a, b) } else { (b, a) };
            if normal.face != 0 || spell.face != 1 { return Err("a preparation card requires normal and inset characteristics (722.2)".into()); }
            return Self::preparation(Self::read_face(normal)?, Self::read_face(spell)?);
        }
        if let [a, b] = faces && a.layout == "adventure" && b.layout == "adventure" && a.oracle_id == b.oracle_id {
            let (normal, adventure) = if a.face <= b.face { (a, b) } else { (b, a) };
            if normal.face != 0 || adventure.face != 1 { return Err("an adventurer card requires normal and alternative characteristics (715.2)".into()); }
            let normal = Self::read_face(normal)?;
            let alternative = Self::read_face(adventure)?;
            return if alternative.subtypes.contains(&ir::Subtype::omen()) {
                Self::omen_card(normal, alternative)
            } else { Self::adventurer(normal, alternative) };
        }
        if let [a, b] = faces && a.layout == "modal_dfc" && b.layout == "modal_dfc" && a.oracle_id == b.oracle_id {
            let (front, back) = if a.face <= b.face { (a, b) } else { (b, a) };
            if front.face != 0 || back.face != 1 { return Err("a modal card has a front and back face (712.3)".into()); }
            return Self::modal(Self::read_face(front)?, Self::read_face(back)?);
        }
        if let [a, b] = faces && a.layout == "transform" && b.layout == "transform" && a.oracle_id == b.oracle_id {
            let (front, back) = if a.face <= b.face { (a, b) } else { (b, a) };
            if front.face != 0 || back.face != 1 { return Err("a double-faced card has a front and a back face (712.1)".into()); }
            return Self::transforming(Self::read_face(front)?, Self::read_face(back)?);
        }
        if faces.len() != 2 || faces.iter().any(|f| f.layout != "split" || f.name != faces[0].name || f.oracle_id != faces[0].oracle_id) {
            return Err(format!("unsupported whole-card layout: {}", faces.first().map_or("empty", |f| f.layout.as_str())));
        }
        let mut faces = faces.to_vec();
        faces.sort_by_key(|f| f.face);
        if faces[0].face != 0 || faces[1].face != 1 { return Err("a split card has a left and a right half (709.1)".into()); }
        Self::split([Self::read_face(faces[0])?, Self::read_face(faces[1])?])
    }

    /// Normal and alternative spell characteristics share selection and copiable values
    /// (715.2–4, 720.2–4); their resolution destinations remain distinct.
    fn with_alternative(mut normal: CardDef, mut alternative: CardDef, choice: CastAs) -> Result<CardDef, String> {
        let subtype = match choice { CastAs::Adventure => ir::Subtype::adventure(), CastAs::Omen => ir::Subtype::omen(), _ => unreachable!() };
        if [&normal, &alternative].iter().any(|d| !d.halves.is_empty() || d.back.is_some() || d.alternative_spell.is_some() || d.prepare_spell.is_some())
            || normal.subtypes.contains(&ir::Subtype::adventure()) || normal.subtypes.contains(&ir::Subtype::omen())
            || !alternative.subtypes.contains(&subtype)
            || !(alternative.is(CardType::Instant) || alternative.is(CardType::Sorcery)) {
            return Err("one normal definition and one instant or sorcery alternative are required (715.2, 720.2)".into());
        }
        alternative.admission = crate::play::check_alternative_spell(&alternative);
        normal.alternative_spell = Some(Box::new(alternative));
        normal.admission = crate::play::validate_definition(&normal);
        Ok(normal)
    }

    /// Preparation retains alternative characteristics but grants no physical-card cast
    /// choice. The inset becomes a generated copy's normal definition (722.3c).
    pub fn preparation(mut normal: CardDef, mut spell: CardDef) -> Result<CardDef, String> {
        if [&normal, &spell].iter().any(|d| !d.halves.is_empty() || d.back.is_some() || d.alternative_spell.is_some() || d.prepare_spell.is_some())
            || !(spell.is(CardType::Instant) || spell.is(CardType::Sorcery)) {
            return Err("one normal definition and one instant or sorcery prepare spell are required (722.2)".into());
        }
        spell.is_prepare_spell = true;
        normal.prepare_spell = Some(Box::new(spell));
        normal.admission = crate::play::validate_definition(&normal);
        Ok(normal)
    }
    pub fn prepare_spell(&self) -> Option<&CardDef> { self.prepare_spell.as_deref() }

    pub fn adventurer(normal: CardDef, alternative: CardDef) -> Result<CardDef, String> {
        Self::with_alternative(normal, alternative, CastAs::Adventure)
    }
    pub fn omen_card(normal: CardDef, alternative: CardDef) -> Result<CardDef, String> {
        Self::with_alternative(normal, alternative, CastAs::Omen)
    }
    pub(crate) fn alternative_spell(&self) -> Option<&CardDef> { self.alternative_spell.as_deref() }
    pub fn adventure(&self) -> Option<&CardDef> {
        self.alternative_spell().filter(|d| d.subtypes.contains(&ir::Subtype::adventure()))
    }
    pub fn omen(&self) -> Option<&CardDef> {
        self.alternative_spell().filter(|d| d.subtypes.contains(&ir::Subtype::omen()))
    }
    pub(crate) fn has_adventure(&self) -> bool {
        self.adventure().is_some() || self.subtypes.contains(&ir::Subtype::adventure())
    }

    /// A nonmodal double-faced card from its two faces (712.2): `front` is the card wherever
    /// it is, and `back` its transformed face, including entry transformed (712.14a).
    pub fn transforming(mut front: CardDef, mut back: CardDef) -> Result<CardDef, String> {
        if [&front, &back].iter().any(|face| !face.halves.is_empty() || face.back.is_some() || face.alternative_spell.is_some() || face.prepare_spell.is_some()) {
            return Err("a face of a double-faced card is one face (712.1)".into());
        }
        if back.cost.is_some() { return Err("the back face of a nonmodal double-faced card has no mana cost (712.2, 202.3b)".into()); }
        back.front_mana_value = Some(front.mana_value());
        // Validate the back face independently, including its own entry choices.
        back.admission = crate::play::validate_definition(&back);
        front.back = Some(Box::new(back));
        // the card is one the engine runs when it runs both of its faces
        front.admission = crate::play::validate_definition(&front);
        Ok(front)
    }

    /// One physical modal card, with front characteristics outside the stack/battlefield.
    pub fn modal(mut front: CardDef, mut back: CardDef) -> Result<CardDef, String> {
        if [&front, &back].iter().any(|d| !d.halves.is_empty() || d.back.is_some() || d.alternative_spell.is_some() || d.prepare_spell.is_some()) {
            return Err("a modal face must be one ordinary face (712.3)".into());
        }
        back.admission = crate::play::validate_definition(&back);
        front.modal = true;
        front.back = Some(Box::new(back));
        front.admission = crate::play::validate_definition(&front);
        Ok(front)
    }

    pub fn modal_face(&self, face: usize) -> Result<&CardDef, String> {
        if !self.modal { return Err("this is not a modal double-faced card (712.3)".into()); }
        match face { 0 => Ok(self), 1 => Ok(self.back.as_deref().expect("modal back face")), _ => Err("choose front face 0 or back face 1 (712.11–12)".into()) }
    }


    /// The back face of a double-faced card; none for any other.
    pub fn back(&self) -> Option<&CardDef> { self.back.as_deref() }

    pub fn split(mut halves: [CardDef; 2]) -> Result<CardDef, String> {
        if halves.iter().any(|h| !h.halves.is_empty() || h.back.is_some() || h.alternative_spell.is_some() || h.prepare_spell.is_some()) { return Err("a split half cannot itself be a split card".into()); }
        for (door, half) in halves.iter_mut().enumerate() { half.door = Some(door); }
        let [left, right] = &halves;
        // a permanent split card has one shared type line (709.5): with neither half unlocked
        // it has no name, mana cost or rules text, only that line
        let room = (!left.is(CardType::Instant) && !left.is(CardType::Sorcery)).then(|| Box::new(Self::compile(Printed {
            halves: Vec::new(), name: String::new(), cost: None, supertypes: left.supertypes.clone(), types: left.types.clone(),
            subtypes: left.subtypes.clone(), colors: Vec::new(), text_colors: Vec::new(), pt: None, loyalty: None, abilities: Vec::new(), modes: Vec::new(),
        })));
        let forms = Forms {
            abilities: left.forms.abilities.iter().chain(&right.forms.abilities).cloned().collect(),
            modes: left.forms.modes.iter().chain(&right.forms.modes).cloned().collect(),
        };
        fn union<T: Copy + PartialEq>(left: &[T], right: &[T]) -> Vec<T> {
            let mut all = left.to_vec();
            for x in right { if !all.contains(x) { all.push(*x); } }
            all
        }
        let printed = Printed {
            name: format!("{} // {}", left.name, right.name),
            cost: match (&left.cost, &right.cost) {
                (Some(a), Some(b)) => Some(ManaCost(a.0.iter().chain(&b.0).copied().collect())),
                _ => None,
            },
            supertypes: union(&left.supertypes, &right.supertypes),
            types: union(&left.types, &right.types),
            subtypes: union(&left.subtypes, &right.subtypes),
            colors: union(&left.colors, &right.colors),
            text_colors: union(&left.text_colors, &right.text_colors),
            pt: left.pt,
            loyalty: left.loyalty,
            abilities: left.declaration_abilities.iter().chain(&right.declaration_abilities).cloned().collect(),
            modes: left.modes.iter().chain(&right.modes).cloned().collect(),
            halves: halves.into(),
        };
        let mut def = Self::compile(printed).with_forms(forms);
        def.room = room;
        Ok(def)
    }

    /// The named optional additional cost compiled for this spell face.
    pub fn optional_cost_kind(&self) -> Option<crate::keyword::Optional> {
        self.costs.optional.first().map(|(_, kind, _)| *kind)
    }

    /// The chosen half on the stack, or the combined characteristics of a fused spell.
    pub fn spell(&self, choice: CastAs) -> Result<&CardDef, String> {
        match choice {
            CastAs::Whole if self.halves.is_empty() => Ok(self),
            CastAs::Face(i) => self.modal_face(i),
            CastAs::Adventure => self.adventure().ok_or_else(|| "this card has no Adventure (715.2)".into()),
            CastAs::Omen => self.omen().ok_or_else(|| "this card has no Omen (720.2)".into()),
            CastAs::Half(i) => self.halves.get(i).ok_or_else(|| "no such split half (709.3)".into()),
            CastAs::Fused if !self.halves.is_empty() && self.fuse() => Ok(self),
            _ => Err("choose one split half, or both with fuse (709.3, 702.102)".into()),
        }
    }

    pub fn fuse(&self) -> bool { crate::keyword::contributions(self).any(|(_, g)| matches!(g, crate::keyword::Gives::Fuse)) }

    pub(crate) fn cast_choices(&self, hand: bool) -> Vec<CastAs> {
        if self.modal { return (0..2).filter(|i|!self.modal_face(*i).unwrap().is(CardType::Land)).map(CastAs::Face).collect(); }
        if let Some(alternative) = self.alternative_spell() {
            return vec![CastAs::Whole, if alternative.subtypes.contains(&ir::Subtype::omen()) { CastAs::Omen } else { CastAs::Adventure }];
        }
        if self.halves.is_empty() { return vec![CastAs::Whole]; }
        let mut choices: Vec<_> = (0..self.halves.len()).map(CastAs::Half).collect();
        if hand && self.fuse() { choices.push(CastAs::Fused); }
        choices
    }

    pub(crate) fn chars(&self) -> crate::state::Chars {
        crate::state::Chars {
            name: self.name.clone(), supertypes: self.supertypes.clone(), types: self.types.clone(),
            subtypes: self.subtypes.clone(), colors: self.colors.clone(), pt: self.pt,
            mana_value: self.mana_value(), keywords: Vec::new(), predefined: None,
        }
    }

    pub fn is(&self, t: CardType) -> bool {
        self.types.contains(&t)
    }

    /// The definition of one ability by itself, of no card: what an effect gives in quotes
    /// (113.2c) or a rule gives (305.6). It has no name, cost or types of its own; what its
    /// words say of "this creature" is said of whatever has the ability.
    pub(crate) fn of_ability(ability: Ability, modes: Vec<Mode>) -> CardDef {
        Self::compile(Printed { halves: Vec::new(), name: String::new(), cost: None, supertypes: Vec::new(), types: Vec::new(), subtypes: Vec::new(), colors: Vec::new(), text_colors: Vec::new(), pt: None, loyalty: None, abilities: vec![ability], modes: vec![modes] })
    }

    /// Apply supported copy exceptions to typed printed parts, never to evaluated layers.
    pub(crate) fn copied(chars: &crate::state::Chars, def: Option<&CardDef>, changes: &[ir::CopyChange]) -> CardDef {
        let mut printed = Printed {
            halves: Vec::new(), name: chars.name.clone(), cost: def.and_then(|d| d.cost.clone()),
            supertypes: chars.supertypes.clone(), types: chars.types.clone(), subtypes: chars.subtypes.clone(),
            colors: chars.colors.clone(), text_colors: def.map(|d| d.text_colors.clone()).unwrap_or_default(), pt: chars.pt, loyalty: def.and_then(|d| d.loyalty),
            abilities: def.map(|d| d.declaration_abilities.clone()).unwrap_or_else(|| chars.keywords.iter().map(|k|
                Ability::Keywords(vec![ir::token::KeywordInstance { keyword: *k, param: ir::token::KeywordParam::None }])).collect()),
            modes: def.map(|d| d.modes.clone()).unwrap_or_default(),
        };
        for change in changes {
            match change {
                ir::CopyChange::Is(ir::action::Becoming::Is(c)) => {
                    for t in &c.types { if !printed.types.contains(t) { printed.types.push(*t); } }
                    for t in &c.subtypes { if !printed.subtypes.contains(t) { printed.subtypes.push(*t); } }
                    for t in &c.supertypes { if !printed.supertypes.contains(t) { printed.supertypes.push(*t); } }
                },
                ir::CopyChange::NotLegendary => printed.supertypes.retain(|s| *s != ir::Supertype::Legendary),
                &ir::CopyChange::Is(ir::action::Becoming::Pt(ir::token::Pt::Const(power), ir::token::Pt::Const(toughness))) =>
                    printed.pt = Some((power as i32, toughness as i32)),
                ir::CopyChange::Has(abilities) => for a in abilities {
                    printed.abilities.push(match a {
                        ir::action::Granted::Ability(a) => *a.clone(),
                        ir::action::Granted::Keyword(k) => Ability::Keywords(vec![*k.clone()]),
                        _ => unreachable!("validated copy addition"),
                    });
                    printed.modes.push(Vec::new());
                },
                _ => unreachable!("validated copy exception"),
            }
        }
        let mut copied = Self::compile(printed.clone());
        // a copy given a power and toughness does not copy the abilities that define them
        // (707.9d)
        if changes.iter().any(|c| matches!(c, ir::CopyChange::Is(ir::action::Becoming::Pt(..)))) {
            let defining: Vec<usize> = copied.characteristics.iter()
                .filter(|r| r.result.as_ref().is_ok_and(|c| c.changes_in(crate::layers::Layer::DefinePt))).map(|r| r.ability).collect();
            if !defining.is_empty() {
                let keep = |i: &usize| !defining.contains(i);
                printed.abilities = printed.abilities.into_iter().enumerate().filter(|(i, _)| keep(i)).map(|(_, a)| a).collect();
                printed.modes = printed.modes.into_iter().enumerate().filter(|(i, _)| keep(i)).map(|(_, m)| m).collect();
                copied = Self::compile(printed);
            }
        }
        // Alternative characteristics are copiable even while the normal ones apply (715.2b).
        copied.prepare_spell = def.and_then(|d| d.prepare_spell.clone());
        copied.is_prepare_spell = def.is_some_and(|d| d.is_prepare_spell);
        copied.alternative_spell = def.and_then(|d| d.alternative_spell.clone());
        if copied.prepare_spell.is_some() || copied.alternative_spell.is_some() { copied.admission = crate::play::validate_definition(&copied); }
        copied
    }

    /// Can a static ability add or remove effective definition groups?
    pub(crate) fn changes_membership(&self) -> bool {
        self.characteristics.iter().any(|r| r.result.as_ref().is_ok_and(|c| c.changes_membership()))
    }

    /// Does the engine hold in force the restriction or requirement the ability at `index`
    /// states? None: that ability is not one. Read from what was lowered when this
    /// definition was loaded; nothing is lowered here.
    pub(crate) fn restriction_ok(&self, index: usize) -> Option<crate::Res<()>> {
        self.restrictions.iter().find(|r| r.ability == index).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }

    /// Does the engine apply what the ability at `index` says its subject is? None: that
    /// ability is not such a statement. Read from what was lowered when this definition was
    /// loaded; nothing is lowered here.
    pub(crate) fn characteristic_ok(&self, index: usize) -> Option<crate::Res<()>> {
        self.characteristics.iter().find(|r| r.ability == index).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }

    /// Does the engine apply the replacement effect the ability at `index` is: a choice made
    /// as its permanent enters, how permanents enter, a life-gain multiplier or a change of
    /// counters put? None: that ability is none of these. Read from what was lowered when
    /// this definition was loaded; nothing is lowered here.
    pub(crate) fn replacement_ok(&self, index: usize) -> Option<crate::Res<()>> {
        self.replacements.iter().find(|r| r.ability == index).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }

    /// Does the engine watch for what the ability at `index` triggers on, its intervening
    /// "if" included? None: that ability is not a triggered ability. Read from what was
    /// lowered when this definition was loaded; nothing is lowered here.
    pub(crate) fn trigger_ok(&self, index: usize) -> Option<crate::Res<()>> {
        self.triggers.iter().find(|r| r.ability == index).map(|r| r.result.as_ref().map(|_| ()).map_err(Clone::clone))
    }

    /// The mode rows under this card's ability at `index` (700.2); none for an ability that
    /// is not modal.
    pub fn modes_of(&self, index: usize) -> &[Mode] {
        self.modes.get(index).map_or(&[], |m| m.as_slice())
    }

    /// Keyword-prefixed declarations stay visible when their executable meaning is lowered.
    pub(crate) fn activation_declaration(&self, index: usize) -> Option<&Ability> {
        self.activation_keywords.get(index).copied().flatten()?;
        self.declaration_abilities.get(index)
    }

    /// The printed row of an ability, including an activated keyword's derived meaning.
    pub fn ability_row(&self, index: usize) -> usize {
        index.checked_sub(self.abilities.len())
            .and_then(|i| self.keyword_abilities.get(i))
            .map_or(index, |(row, _)| *row)
    }

    /// Whether this instant's or sorcery's instructions read an X announced as it is cast:
    /// a cost it can be cast for has {X} (107.3a).
    pub(crate) fn spell_x(&self) -> bool {
        let x = |cost: &ManaCost| cost.0.contains(&crate::cost::X);
        (self.is(CardType::Instant) || self.is(CardType::Sorcery))
            && (self.cost.as_ref().is_some_and(x) || crate::keyword::flashback_mana(self).as_ref().is_some_and(x)
                || self.casting_costs().any(|(_, cost)| cost.0.iter().any(|p| matches!(p, ir::CostPart::Action(ir::Action::PayLife(ir::Amount::X)))
                    || matches!(p, ir::CostPart::Action(ir::Action::Waterbend(cost)) if x(cost)))))
    }

    /// A permanent card with {X} in its mana cost: the spell it is cast as chooses X, which
    /// the permanent's enters abilities read (107.3m).
    pub(crate) fn permanent_x(&self) -> bool {
        !(self.is(CardType::Instant) || self.is(CardType::Sorcery)) && self.cost.as_ref().is_some_and(|cost| cost.0.contains(&crate::cost::X))
    }

    /// The announced spell's mana value includes each chosen X (202.3e).
    pub(crate) fn spell_mana_value(&self, x: Option<u32>) -> u32 {
        let symbols=self.cost.as_ref().map_or(0,|c|c.0.iter().filter(|s|**s==crate::cost::X).count() as u32);
        self.mana_value().saturating_add(x.unwrap_or(0).saturating_mul(symbols))
    }
    /// The total of the mana cost (202.3); X is 0 off the stack.
    pub fn mana_value(&self) -> u32 {
        if let Some(front) = self.front_mana_value { return front; }
        self.cost.as_ref().map_or(0, |c| {
            c.0.iter()
                .map(|s| match s {
                    ManaSymbol::Generic(n) => *n,
                    ManaSymbol::Var(_) => 0,
                    ManaSymbol::TwoHybrid(_) => 2,
                    _ => 1,
                })
                .sum()
        })
    }
}

/// What a card is cast as, announced before it moves to the stack (709.3, 702.102a).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CastAs {
    #[default]
    Whole,
    Half(usize),
    Fused,
    Adventure,
    Omen,
    /// Chosen face of a modal double-faced card (712.11b).
    Face(usize),
}

/// The abilities of a face, one per printed ability row, and the mode rows under each.
/// The Oracle text's words for each "~" and "§" it prints, in order: of each ability's own
/// row, and of each mode row under it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Forms {
    abilities: Vec<Vec<String>>,
    modes: Vec<Vec<Vec<String>>>,
}

/// A face's text as read: its abilities, the mode rows under each, the colors of its mana
/// symbols, its self-references' words, and whether it took a station card's power and
/// toughness.
type Read = (Vec<Ability>, Vec<Vec<Mode>>, Vec<Color>, Forms, bool);

/// `pt`: the printed power and toughness, which a station card's highest symbol has (721.2b);
/// the last value says whether it took them. `unread`, collecting: each row is read on its
/// own (113.2c), so a row the grammar does not read is kept there and the next one read; the
/// text is then refused with the first.
fn read_as(text: &str, name: &str, face_name: &str, type_line: &str, pt: Option<(i32, i32)>, mut unread: Option<&mut Vec<crate::play::Blocker>>) -> Result<Read, String> {
    let n = normalize(text, name, face_name, type_line);
    // each row's words for the objects it names as "~" and "§", in text order
    let mut refs = n.self_refs().iter();
    let row_forms: Vec<Vec<String>> = n.rows.iter().map(|row| row.chars()
        .filter(|c| *c == oracle::normalize::SELF || *c == oracle::normalize::GRANTING_SELF)
        .filter_map(|_| refs.next().cloned()).collect()).collect();
    let mut forms = Vec::new();
    // a modal ability's rows', by ability and mode; level symbols and class bars keep the
    // abilities printed before them, modal ones among them, where they were
    let mut mode_forms: Vec<Vec<Vec<String>>> = Vec::new();
    let ab = abilities();
    let (mut out, mut modes) = (Vec::new(), Vec::new());
    let mut symbols = Vec::new();
    fn collect_symbols(tokens: &[oracle::token::Token], out: &mut Vec<ManaSymbol>) {
        for token in tokens {
            match &token.kind {
                Kind::Symbol(s) => { if let Some(symbol) = ManaSymbol::parse(s) { out.push(symbol); } }
                Kind::Quote { body, .. } => collect_symbols(body, out),
                _ => {}
            }
        }
    }
    let before = unread.as_ref().map_or(0, |all| all.len());
    for b in &classify(&n) {
        if b.kind == Row::Empty {
            continue;
        }
        for (i, r) in b.rows.clone().enumerate() {
            let toks = tokenize(&n.rows[r]);
            collect_symbols(&toks, &mut symbols);
            if toks.is_empty() {
                continue;
            }
            let row = || crate::play::UnreadRow { index: r, mode: i > 0, text: n.rows[r].clone(), stop: None };
            if i == 0 {
                if let Some(ability) = read(ab.ability.parse_all(&toks), &toks, row, unread.as_deref_mut())? { out.push(ability); }
                modes.push(Vec::new());
                forms.push(row_forms[r].clone());
            } else {
                // a row under the ability's own, one of its modes (700.2)
                let mode = read(ab.mode.parse_all(&toks), &toks, row, unread.as_deref_mut())?;
                let Some(under) = modes.last_mut() else {
                    let Some(all) = unread.as_deref_mut() else { return Err("a mode row under no ability".into()) };
                    all.push(crate::play::Blocker::parse("a mode row under no ability"));
                    continue;
                };
                if let Some(mode) = mode { under.push(mode); }
                mode_forms.resize(modes.len(), Vec::new());
                mode_forms[modes.len() - 1].push(row_forms[r].clone());
            }
        }
    }
    if let Some(first) = unread.as_ref().and_then(|all| all.get(before)) { return Err(first.reason.clone()); }
    // a leveler's level symbols are the static abilities they stand for (711.2)
    let (out, modes, forms) = crate::level::read(out, modes, forms);
    // a Class's level bars are the abilities they stand for (716.2)
    let (out, modes, forms) = crate::class::read(out, modes, forms);
    // a station card's station symbols are the static abilities they stand for (721.2)
    let station = crate::station::takes_pt(&out) && pt.is_some();
    let (out, modes, forms) = crate::station::read(out, modes, forms, pt);
    let station = station && !crate::station::takes_pt(&out);
    mode_forms.truncate(out.len());
    Ok((out, modes, colors_of(&ManaCost(symbols)), Forms { abilities: forms, modes: mode_forms }, station))
}

/// A text row's reading; or its refusal, returned, or, collecting (`unread`), kept with where
/// the grammar stopped, and `None`.
fn read<T>(outcome: Outcome<T>, toks: &[oracle::token::Token], row: impl FnOnce() -> crate::play::UnreadRow, unread: Option<&mut Vec<crate::play::Blocker>>) -> Result<Option<T>, String> {
    let (reason, stop) = match outcome {
        Outcome::Accepted { value, .. } => return Ok(Some(value)),
        Outcome::Ambiguous(_) => ("ambiguous text", None),
        Outcome::Declined { at, expected, .. } => ("text the grammar does not read", Some((at.min(toks.len()), expected))),
    };
    let Some(all) = unread else { return Err(reason.to_string()) };
    let unspaced = |toks: &[oracle::token::Token]| {
        let mut toks = toks.to_vec();
        if let Some(first) = toks.first_mut() { first.space_before = 0; }
        detokenize(&toks)
    };
    let stop = stop.map(|(at, expected)| crate::play::Stop {
        read: unspaced(&toks[..at]),
        at: toks.get(at).map_or("<end>".to_string(), |t| unspaced(std::slice::from_ref(t))),
        rest: unspaced(&toks[at..]),
        expected,
    });
    let row = crate::play::UnreadRow { stop, ..row() };
    // the grammar read no further than its stop: what follows it, of no rule yet
    let unanalyzed = row.stop.iter().filter(|s| !s.rest.is_empty()).map(|s| format!("the rest of row {}: {:?}", row.index, s.rest)).collect();
    all.push(crate::play::Blocker { row: Some(row), unanalyzed, ..crate::play::Blocker::parse(reason) });
    Ok(None)
}

/// A station card's power and toughness box: numbers printed in its symbol's striation.
fn boxed_pt(f: &oracle::corpus::Face) -> Option<(i32, i32)> {
    f.power.as_deref().and_then(|p| p.parse().ok()).zip(f.toughness.as_deref().and_then(|t| t.parse().ok()))
}

/// A printed power or toughness (208.1). The star's defining ability among `abilities`
/// supplies its value (208.2a); a printed offset remains part of the base characteristics if
/// that ability is lost.
fn printed_number(text: &str, power: bool, abilities: &[Ability]) -> Result<i32, String> {
    let defined = abilities.iter().filter_map(crate::program::registration::defining_pt)
        .any(|(p, t)| if power { p.is_some() } else { t.is_some() });
    let printed = if defined { text.replace('*', "0") } else { text.to_owned() };
    printed.split('+').try_fold(0i32, |sum, term| {
        term.parse::<i32>().ok().and_then(|n| sum.checked_add(n))
            .ok_or_else(|| format!("{} {text}", if power { "power" } else { "toughness" }))
    })
}

/// The colors of a color indicator (204.2).
fn indicated_colors(indicator: &[String]) -> Result<Vec<Color>, String> {
    if indicator.is_empty() { return Err("a color indicator denotes one or more colors (204.1)".into()); }
    let mut colors = Vec::new();
    for symbol in indicator {
        let Some(ManaSymbol::Color(color)) = ManaSymbol::parse(symbol) else {
            return Err(format!("invalid color indicator {symbol:?} (105.1, 204.2)"));
        };
        if !colors.contains(&color) { colors.push(color); }
    }
    Ok(colors)
}

fn mana_cost_of(text: &str) -> Result<Option<ManaCost>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    let symbols = tokenize(text)
        .iter()
        .map(|t| match &t.kind {
            Kind::Symbol(s) => ManaSymbol::parse(s).ok_or_else(|| format!("mana symbol {{{s}}}")),
            other => Err(format!("mana cost {other:?}")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(ManaCost(symbols)))
}

fn colors_of(cost: &ManaCost) -> Vec<Color> {
    let mut colors = Vec::new();
    let mut add = |c: Color| {
        if !colors.contains(&c) {
            colors.push(c)
        }
    };
    for s in &cost.0 {
        match *s {
            ManaSymbol::Color(c) | ManaSymbol::TwoHybrid(c) | ManaSymbol::ColorlessHybrid(c) | ManaSymbol::Phyrexian(c) => add(c),
            ManaSymbol::Hybrid(a, b) | ManaSymbol::HybridPhyrexian(a, b) => {
                add(a);
                add(b)
            }
            ManaSymbol::Generic(_) | ManaSymbol::Var(_) | ManaSymbol::Colorless | ManaSymbol::Snow => {}
        }
    }
    colors
}

/// "Legendary Creature — Elf Druid" as supertypes, card types and subtypes (205.1).
fn type_line_parts(line: &str) -> Result<(Vec<Supertype>, Vec<CardType>, Vec<Subtype>), String> {
    let (left, right) = line.split_once('—').unwrap_or((line, ""));
    let (mut supertypes, mut types) = (Vec::new(), Vec::new());
    for w in left.split_whitespace() {
        let l = w.to_lowercase();
        if let Some(s) = Supertype::from_word(&l) {
            supertypes.push(s);
        } else if let Some(t) = CardType::from_word(&l) {
            types.push(t);
        } else {
            return Err(format!("type {w}"));
        }
    }
    let subtypes = right
        .split_whitespace()
        .map(|w| Subtype::from_name(w).ok_or_else(|| format!("subtype {w}")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((supertypes, types, subtypes))
}
