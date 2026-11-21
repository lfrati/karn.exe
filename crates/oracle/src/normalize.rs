//! Oracle text → ability rows the grammar reads, **and back**.
//!
//! The grammar's round trip compares printed rows against these, so every pass here must be
//! invertible: what it removes or rewrites is recorded, and [`Normalized::restore_rows`] replays
//! the inverses in reverse order to rebuild the original bytes. A pass that threw information away
//! would let the round trip pass by normalizing harder.
//!
//! | Pass | Forward | Inverse |
//! |---|---|---|
//! | Reminder text | strip `(…)` spans with the spaces before them | re-insert at the recorded offsets |
//! | Rows | split on `\n` (bullets stay separate rows; [`crate::classify`] groups them) | join with `\n` |
//! | Labels | strip a row's leading ability word (CR 207.2c) or flavor word (207.2d) and its ` — ` | put it back on that row |
//! | Self-reference | the card's names and "this creature"-style nouns → [`SELF`]; a name inside a quoted ability → [`GRANTING_SELF`] | put the recorded surface form back |
//!
//! Symbols (`{T}`, `{2}{U}`) are not rewritten here; the [`crate::token`] lexer reads them, so
//! nothing moves and there is no inverse to get wrong. The design follows argentum's Assay
//! normalizer (`argentum/oracle-assay/.../normalize/Normalizer.kt`).

use crate::{cr, names};

/// A reference to the object the ability is on: the card's own name, its CR 201.5c short name,
/// or "this creature" / "this spell" / "this Aura" …. Oracle text never contains a tilde.
pub const SELF: char = '~';

/// A card naming itself **inside a quoted ability it grants**. CR 201.5a: that name means the
/// granting object, while "this creature" in the same quote means the object that has the
/// ability (Trusty Boomerang returns the Equipment, not the creature). Never in Oracle text.
pub const GRANTING_SELF: char = '§';

/// Self-reference nouns: "this creature", "This spell", …. Rooms' "this door" is a half of the
/// card, not the card, and stays text.
const SELF_NOUNS: &[&str] = &[
    "creature",
    "spell",
    "artifact",
    "land",
    "enchantment",
    "card",
    "Aura",
    "token",
    "Equipment",
    "Vehicle",
    "Saga",
    "permanent",
    "Siege",
    "Class",
    "Spacecraft",
    "Case",
    "planeswalker",
    "Room",
    "emblem",
    "battle",
];

/// What a row's leading label was. Neither has rules meaning (CR 207.2c, 207.2d); the kind is
/// kept for statistics and for printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LabelKind {
    /// Listed in CR 207.2c ("Landfall", "Council's dilemma").
    AbilityWord,
    /// Tailored to one card ("Siege Monster", "Tunnel Snakes Rule!"), recognized by shape.
    FlavorWord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub kind: LabelKind,
    pub text: String,
    /// What the row printed before the label: "" for a label that starts the row, "• " for a
    /// mode, "I, II — " for chapters, "1—9 | " for a die-roll result (CR 706.3a). The label goes
    /// back right after it.
    pub after: String,
}

/// A span removed from the text, and the offset in the stripped text to put it back at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removal {
    pub offset: usize,
    pub text: String,
}

/// A face's Oracle text as rows, carrying everything needed to undo the normalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalized {
    /// One per printed row. A row that held only reminder text is empty.
    pub rows: Vec<String>,
    /// The label stripped from each row, positionally.
    pub labels: Vec<Option<Label>>,
    /// Everything taken out of the text and put back by offset, left to right: the reminder text,
    /// and for a spell with cleave (702.148a) the square brackets themselves, whose words stay in
    /// the rows (the spell's text as it is cast without paying its cleave cost).
    reminders: Vec<Removal>,
    /// The rows when the cleave cost was paid: the bracketed words are removed.
    cleaved: Option<Box<Normalized>>,
    self_refs: Vec<String>,
}

impl Normalized {
    /// The same rows with the text in square brackets removed, for a spell with cleave (702.148a,
    /// a text-changing effect, 612). A face without cleave has none.
    pub fn cleaved(&self) -> Option<&Normalized> {
        self.cleaved.as_deref()
    }

    /// The reminder text removed (and a cleave spell's square brackets), in order, with offsets
    /// into the stripped text.
    pub fn reminders(&self) -> &[Removal] {
        &self.reminders
    }

    /// The surface form of each [`SELF`] / [`GRANTING_SELF`], in text order.
    pub fn self_refs(&self) -> &[String] {
        &self.self_refs
    }

    /// The original Oracle text.
    pub fn restore(&self) -> String {
        self.restore_rows(&self.rows)
    }

    /// The Oracle text these printed rows denote. With `rows == self.rows` this must equal the
    /// input byte for byte; with rows printed by the grammar it is the round-trip check. A row
    /// count that differs from the recorded one leaves labels off, and the caller's comparison
    /// fails.
    pub fn restore_rows(&self, rows: &[String]) -> String {
        let mut refs = self.self_refs.iter();
        let mut out = Vec::with_capacity(rows.len());
        for (i, row) in rows.iter().enumerate() {
            let mut line = String::with_capacity(row.len());
            for c in row.chars() {
                match (c, (c == SELF || c == GRANTING_SELF).then(|| refs.next()).flatten()) {
                    (_, Some(form)) => line.push_str(form),
                    (c, None) => line.push(c),
                }
            }
            let mut line = typographic_commas(&line);
            if rows.len() == self.labels.len()
                && let Some(label) = &self.labels[i]
            {
                // a row printed without the prefix it was read with cannot take the label back
                if let Some(rest) = line.strip_prefix(label.after.as_str()) {
                    line = format!("{}{}{LABEL_DASH}{rest}", label.after, label.text);
                }
            }
            out.push(line);
        }
        let mut text = out.join("\n");
        for r in self.reminders.iter().rev() {
            if r.offset > text.len() || !text.is_char_boundary(r.offset) {
                return text; // printed text diverged; the caller's comparison fails
            }
            text.insert_str(r.offset, &r.text);
        }
        text
    }
}

const LABEL_DASH: &str = " — ";

/// Oracle puts the comma of an outer sentence inside the closing quotation mark (`with "{T}:
/// Add {G}," and it loses …`); the grammar reads it outside, where it belongs to the sentence.
/// Exact both ways: the corpus never has a comma right after a closing quotation mark.
fn logical_commas(row: &str) -> String {
    row.replace(",\"", "\",")
}

/// The inverse of [`logical_commas`].
fn typographic_commas(row: &str) -> String {
    row.replace("\",", ",\"")
}

/// Normalize one face. `name` is the card's full name, `face_name` the face's; `type_line`
/// decides whether a shortened name may stand for the card (legendary faces only).
pub fn normalize(oracle_text: &str, name: &str, face_name: &str, type_line: &str) -> Normalized {
    let (stripped, reminders) = strip_reminders(oracle_text);
    let forms = self_forms(name, face_name, type_line.contains("Legendary"));
    // 702.148a: a spell with cleave has text in square brackets, which cleave removes
    let (stripped, removed, cleaved) = if has_cleave(&stripped) {
        let (kept, removed) = strip_brackets(&stripped, reminders);
        let cleaved = remove_bracketed(&stripped);
        (kept, removed, Some(cleaved))
    } else {
        (stripped, reminders, None)
    };
    let mut n = read_rows(&stripped, &forms);
    n.reminders = removed;
    // the text with the bracketed words removed has the same rows, labels and names
    n.cleaved = cleaved.map(|text| Box::new(read_rows(&text, &forms)));
    n
}

/// The rows of a text: labels split off, names abstracted.
fn read_rows(stripped: &str, forms: &[(String, Form)]) -> Normalized {
    let mut rows = Vec::new();
    let mut labels = Vec::new();
    let mut self_refs = Vec::new();
    let all: Vec<&str> = stripped.split('\n').collect();
    for row in &all {
        let (label, body) = split_label(row, &all);
        let body = logical_commas(&body);
        rows.push(abstract_self(&body, forms, &mut self_refs));
        labels.push(label);
    }
    Normalized { rows, labels, reminders: Vec::new(), cleaved: None, self_refs }
}

/// Does a row of the text give the spell cleave (702.148a: "Cleave {2}{R}")?
fn has_cleave(text: &str) -> bool {
    text.split('\n').any(|row| row.starts_with("Cleave {"))
}

/// The text without the square brackets themselves (the words in them stay). The removals are
/// the reminders, moved to offsets in the text returned, and the brackets, each in its place
/// from left to right.
fn strip_brackets(text: &str, reminders: Vec<Removal>) -> (String, Vec<Removal>) {
    let mut out = String::with_capacity(text.len());
    let mut removals = Vec::new();
    let mut pending = reminders.into_iter().peekable();
    for (at, c) in text.char_indices() {
        while let Some(r) = pending.next_if(|r| r.offset == at) {
            removals.push(Removal { offset: out.len(), text: r.text });
        }
        if c == '[' || c == ']' {
            removals.push(Removal { offset: out.len(), text: c.to_string() });
        } else {
            out.push(c);
        }
    }
    for r in pending {
        removals.push(Removal { offset: out.len(), text: r.text });
    }
    (out, removals)
}

/// The text with every span in square brackets removed (702.148a), and the space that stood
/// between it and the rest: "a [basic land] card" is "a card", "creatures [with flying]." is
/// "creatures.", a bracketed last sentence leaves the sentence before it.
fn remove_bracketed(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']').map(|c| open + c) else { break };
        out.push_str(&rest[..open]);
        rest = &rest[close + 1..];
        let next = rest.chars().next();
        if next.is_none_or(|c| matches!(c, '\n' | '.' | ',' | ';' | ':')) {
            // nothing follows to be separated from: the space in front goes
            if out.ends_with(' ') {
                out.pop();
            }
        } else if next == Some(' ') {
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Reminder text: every `(…)` span plus the spaces in front of it (never a newline, so a row of
/// pure reminder text stays an empty row). Oracle never nests parentheses (checked by the gate).
fn strip_reminders(text: &str) -> (String, Vec<Removal>) {
    let mut out = String::with_capacity(text.len());
    let mut removals = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')').map(|c| open + c) else {
            break;
        };
        let before = &rest[..open];
        let kept = before.trim_end_matches(' ');
        out.push_str(kept);
        removals.push(Removal { offset: out.len(), text: rest[kept.len()..=close].to_string() });
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    (out, removals)
}

/// A row's "Label — ", when the label is an ability word or has the shape of a flavor word
/// (CR 207.2c, 207.2d). The label starts the row, or follows the row's own marker (a mode's
/// bullet, a chapter's numerals, a die-roll result's range, see [`marker`]). Returns the row
/// without its label.
///
/// A bullet's leading word may be an anchor word instead (CR 614.12c, "• Khans — …"), which has
/// rules meaning: it is the word another row of the same face makes the player choose ("As ~
/// enters, choose Khans or Dragons."). Such a word is not a label and stays in the row.
fn split_label(row: &str, face: &[&str]) -> (Option<Label>, String) {
    let at = marker(row);
    let (after, rest) = row.split_at(at);
    let Some(dash) = rest.find(LABEL_DASH) else {
        return (None, row.to_string());
    };
    let (prefix, body) = (&rest[..dash], &rest[dash + LABEL_DASH.len()..]);
    let folded = cr::fold(prefix);
    let kind = if cr::ability_words().contains(&folded) {
        LabelKind::AbilityWord
    } else if is_flavor_word(prefix, &folded)
        // a mode's lone keyword action and a dash says no instruction: it names the mode
        // ("• Reveal — Until end of turn, …", Secret Identity)
        || (after.starts_with('•') && cr::keyword_actions().contains(&folded))
    {
        LabelKind::FlavorWord
    } else {
        return (None, row.to_string());
    };
    if after.starts_with('•') && is_chosen(prefix, face) {
        return (None, row.to_string());
    }
    (Some(Label { kind, text: prefix.to_string(), after: after.to_string() }), format!("{after}{body}"))
}

/// Is `word` named by a "choose" instruction in a row that is not a mode (CR 614.12c)?
fn is_chosen(word: &str, face: &[&str]) -> bool {
    face.iter().any(|r| {
        // the word follows the instruction ("choose Khans or Dragons"), not a name before it
        let Some(choose) = r.to_ascii_lowercase().find("choose") else {
            return false;
        };
        !r.starts_with('•')
            && r.match_indices(word).any(|(i, w)| {
                i > choose && !is_name_char(r[..i].chars().next_back()) && !is_name_char(r[i + w.len()..].chars().next())
            })
    })
}

/// The bytes a row spends on its own marker before a label may start: the bullet of a mode
/// ("• "), a chapter's numerals ("I, II — ", CR 714.2b), or a die-roll result's range ("1—9 | ",
/// "20 | ", "15+ | ", CR 706.3a). Zero for any other row.
fn marker(row: &str) -> usize {
    if row.starts_with("• ") {
        return "• ".len();
    }
    let numerals = row.split(LABEL_DASH).next().unwrap_or("");
    if numerals.len() < row.len() && numerals.split(", ").all(is_roman) {
        return numerals.len() + LABEL_DASH.len();
    }
    let digits = |s: &str| s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let mut at = digits(row);
    if at > 0 {
        let rest = &row[at..];
        if let Some(r) = rest.strip_prefix(['—', '–', '-']) {
            at += rest.len() - r.len() + digits(r);
        } else if rest.starts_with('+') {
            at += 1;
        }
        if row[at..].starts_with(" | ") {
            return at + " | ".len();
        }
    }
    0
}

/// CR 207.2d flavor words are unbounded, so they are recognized by shape: a short title-cased
/// phrase that is not a keyword ("Max speed —", "Solved —", "∞ —", CR 702), not a keyword action,
/// not a Case's "To solve —" (719.3a), not a Saga chapter, and not a mode or cost row. Anything
/// with sentence case ("Each opponent faces a villainous choice — …", 701.55a) is text.
fn is_flavor_word(prefix: &str, folded: &str) -> bool {
    const SMALL: &[&str] = &[
        "a", "an", "the", "of", "in", "and", "or", "to", "for", "with", "on", "at", "my", "is", "it", "up", "from",
        "by", "as", "into", "y", "vs.",
    ];
    let title = |w: &str| {
        let first = w.chars().next();
        first.is_some_and(|c| c.is_uppercase() || !c.is_alphabetic()) || SMALL.contains(&w)
    };
    // a keyword, or a keyword with a parameter ("Protection from red —"); a keyword followed by
    // title-cased words is a flavor word that happens to start with one ("Search the Room —",
    // "Play Arcanum —", "Gift of Chaos —")
    let starts_with = |k: &String| {
        folded == k
            || (folded.starts_with(&format!("{k} ")) && !prefix.split(' ').skip(k.split(' ').count()).all(&title))
    };
    if prefix.is_empty()
        || prefix.len() > 60
        || prefix.contains(['{', '[', '•', '∞', '|'])
        || prefix.starts_with('+')
        || !prefix.contains(char::is_alphabetic)
        || folded == "to solve"
        || cr::keyword_abilities().iter().any(starts_with)
        || cr::keyword_actions().iter().any(starts_with)
        || prefix.split([',', ' ']).filter(|w| !w.is_empty()).all(is_roman)
    {
        return false;
    }
    prefix.split(' ').all(title)
}

fn is_roman(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| matches!(c, 'I' | 'V' | 'X'))
}

/// How a self-reference is spelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    /// The name or a comma/"the" short name; inside quotes it becomes [`GRANTING_SELF`].
    Name,
    /// A legendary card's shortened name without a comma ("Rasputin" for "Rasputin
    /// Dreamweaver", "King Darien" for "King Darien XLVIII"): only when no capitalized word
    /// follows, which would make it part of another name ("Soulblade Renewer").
    Short,
    /// "this creature", "this card".
    Noun,
}

/// A name that is only a word of the game: a keyword action ("Regenerate target creature" is the
/// card named Regenerate, CR 701), or, for a name shortened at " the ", also a type or keyword
/// ("Noggle" in "Noggle the Mind" is the creature type, CR 205.3). With no other evidence the word
/// is the word, not a reference to the card. A comma after it ("Storm, Queen of Wakanda") is that
/// evidence: it marks the first part as the name, so the comma shortening is never refused.
/// A planeswalker type is both its type and its name (205.3j).
fn is_game_word(word: &str, whole: bool) -> bool {
    if word.contains(' ') {
        return false;
    }
    let folded = cr::fold(word);
    cr::keyword_actions().contains(&folded)
        || (!whole
            && !cr::planeswalker_types().contains(&folded)
            && (cr::subtypes().contains(&folded) || cr::keyword_abilities().contains(&folded)))
}

/// Surface forms that refer to the card itself, longest first.
fn self_forms(name: &str, face_name: &str, legendary: bool) -> Vec<(String, Form)> {
    let mut names: Vec<String> = Vec::new();
    let add = |n: &str, names: &mut Vec<String>| {
        if n.len() > 1 && !names.iter().any(|x| x == n) {
            names.push(n.to_string());
        }
    };
    for full in [face_name, name] {
        if full.contains("//") {
            continue;
        }
        if !is_game_word(full, true) {
            add(full, &mut names);
        }
        // CR 201.5c: a shortened name ("Akroma" for "Akroma, Angel of Wrath", "Phage" for
        // "Phage the Untouchable") is treated as the full name.
        for sep in [", ", " the "] {
            if let Some(at) = full.find(sep)
                && (sep == ", " || !is_game_word(&full[..at], false))
            {
                add(&full[..at], &mut names);
            }
        }
    }
    let mut forms: Vec<(String, Form)> = names.into_iter().map(|n| (n, Form::Name)).collect();
    if legendary && !face_name.contains("//") {
        let words: Vec<&str> = face_name.split(' ').collect();
        let mut short = Vec::new();
        // "King Darien XLVIII"
        let numeral = |w: &str| w.len() > 1 && w.chars().all(|c| matches!(c, 'I' | 'V' | 'X' | 'L' | 'C' | 'D' | 'M'));
        if words.len() >= 2 && words.last().is_some_and(|w| numeral(w)) {
            short.push(words[..words.len() - 1].join(" "));
        }
        if let Some(at) = face_name.find(" of ") {
            short.push(face_name[..at].to_string());
        }
        // the first word, for names without a comma short name ("Zurgo" for "Zurgo Helmsmasher")
        if words.len() >= 2 && !face_name.contains(", ") {
            short.push(words[0].to_string());
        }
        for n in short {
            let ok = n.chars().count() > 2
                && !n.ends_with("'s")
                && n != "The"
                && n.chars().next().is_some_and(char::is_uppercase)
                && !n.ends_with(',')
                && (!cr::subtypes().contains(&cr::fold(&n)) || cr::planeswalker_types().contains(&cr::fold(&n)))
                && !cr::keyword_abilities().contains(&cr::fold(&n))
                && !cr::keyword_actions().contains(&cr::fold(&n))
                && !forms.iter().any(|(f, _)| *f == n);
            if ok {
                forms.push((n, Form::Short));
            }
        }
    }
    for noun in SELF_NOUNS {
        forms.push((format!("this {noun}"), Form::Noun));
        forms.push((format!("This {noun}"), Form::Noun));
    }
    forms.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    forms
}

/// The next word is capitalized: "Soulblade" in "Partner with Soulblade Renewer" is part of
/// another name.
fn before_capitalized(after: &str) -> bool {
    after.strip_prefix(' ').and_then(|n| n.chars().next()).is_some_and(char::is_uppercase)
}

/// The word before is a determiner or quantifier: a name never takes one, so "each Garruk you
/// control" says the planeswalker type (CR 205.3j), not the card (CR 201.5c).
fn after_determiner(before: &str) -> bool {
    const DETERMINERS: &[&str] = &[
        "a", "an", "the", "each", "every", "another", "other", "target", "that", "those", "these", "this", "any", "all",
        "no", "one", "two", "three", "x", "your", "their", "its",
    ];
    let word: String = before.trim_end_matches(' ').chars().rev().take_while(|c| c.is_alphabetic()).collect();
    let word: String = word.chars().rev().collect::<String>().to_lowercase();
    DETERMINERS.contains(&word.as_str())
}

/// A name followed by a type noun is a subtype ("control a Lukka planeswalker"), not the card.
fn before_type_noun(after: &str) -> bool {
    const NOUNS: &[&str] = &[
        "planeswalker",
        "planeswalkers",
        "creature",
        "creatures",
        "card",
        "cards",
        "permanent",
        "permanents",
        "spell",
        "spells",
    ];
    let next = after.strip_prefix(' ').unwrap_or("");
    let word: String = next.chars().take_while(|c| c.is_alphabetic()).collect();
    NOUNS.contains(&word.as_str())
}

fn is_name_char(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

fn abstract_self(text: &str, forms: &[(String, Form)], recorded: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quoted = false;
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        let prev = text[..i].chars().next_back();
        let hit = if is_name_char(prev) {
            None
        } else {
            forms.iter().find(|(form, kind)| {
                let Some(after) = rest.strip_prefix(form.as_str()) else {
                    return false;
                };
                let rejected = is_name_char(after.chars().next())
                    || (*kind != Form::Noun && before_type_noun(after))
                    || (*kind == Form::Short && before_capitalized(after))
                    || (*kind != Form::Noun
                        && cr::subtypes().contains(&cr::fold(form))
                        && after_determiner(&text[..i]))
                    // CR 201.5: only the text that names the object is a reference; this is
                    // part of another card's name ("Silver Surfer, Galactus's Herald")
                    || (*kind != Form::Noun && names::inside_longer_name(text, i, form.len()));
                !rejected
            })
        };
        if let Some((form, kind)) = hit {
            out.push(if quoted && *kind != Form::Noun { GRANTING_SELF } else { SELF });
            recorded.push(form.clone());
            i += form.len();
        } else {
            let c = rest.chars().next().expect("i < len");
            if c == '"' {
                quoted = !quoted;
            }
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(text: &str, name: &str) -> Normalized {
        roundtrip_typed(text, name, "Creature")
    }

    fn roundtrip_typed(text: &str, name: &str, type_line: &str) -> Normalized {
        let n = normalize(text, name, name, type_line);
        assert_eq!(n.restore(), text);
        n
    }

    #[test]
    fn reminder_and_self() {
        let n = roundtrip(
            "Flying (This creature can't be blocked except by creatures with flying or reach.)\nWhen Serra Angel enters, draw a card.",
            "Serra Angel",
        );
        assert_eq!(n.rows, ["Flying", "When ~ enters, draw a card."]);
    }

    #[test]
    fn reminder_only_row_stays() {
        let n = roundtrip("({T}: Add {G}.)\nThis land enters tapped.", "Some Land");
        assert_eq!(n.rows, ["", "~ enters tapped."]);
    }

    #[test]
    fn short_name_and_possessive() {
        let n = roundtrip("Akroma's power is equal to this creature's toughness.", "Akroma, Angel of Wrath");
        assert_eq!(n.rows, ["~'s power is equal to ~'s toughness."]);
    }

    #[test]
    fn granted_name_inside_quotes() {
        let n = roundtrip(
            "Equipped creature has \"{1}, {T}: Tap target creature. Return Trusty Boomerang to its owner's hand.\"",
            "Trusty Boomerang",
        );
        assert_eq!(n.rows, ["Equipped creature has \"{1}, {T}: Tap target creature. Return § to its owner's hand.\""]);
    }

    #[test]
    fn subtype_named_like_the_card_is_not_self() {
        let n = roundtrip(
            "Creature cards exiled this way gain \"You may cast this card from exile as long as you control a Lukka planeswalker.\"\n−2: Lukka deals 2 damage.",
            "Lukka, Coalescing Hunter",
        );
        assert!(n.rows[0].contains("a Lukka planeswalker"));
        assert_eq!(n.rows[1], "−2: ~ deals 2 damage.");
    }

    #[test]
    fn ability_word_and_flavor_word() {
        let n = roundtrip(
            "Landfall — Whenever a land you control enters, draw a card.\nSiege Monster — When this creature enters, roll a d20.\nTo solve — You control seven or more lands.",
            "X",
        );
        let label = |kind, text: &str| Some(Label { kind, text: text.into(), after: String::new() });
        assert_eq!(n.labels[0], label(LabelKind::AbilityWord, "Landfall"));
        assert_eq!(n.labels[1], label(LabelKind::FlavorWord, "Siege Monster"));
        assert_eq!(n.labels[2], None);
        assert_eq!(n.rows[1], "When ~ enters, roll a d20.");
    }

    #[test]
    fn keyword_and_chapter_prefixes_are_not_labels() {
        let n = roundtrip("I, II — Draw a card.\nMax speed — {T}: Add {R}.\n∞ — ~ has flying.", "X");
        assert!(n.labels.iter().all(Option::is_none));
    }

    /// CR 207.2d: a flavor word may follow a mode's bullet, a chapter's numerals or a die-roll
    /// result's range; the label comes back after that marker.
    #[test]
    fn r207_2d_labels_after_a_marker() {
        let n = roundtrip(
            "Choose one —\n• Bardic Inspiration — Target creature gets +2/+2 until end of turn.\n• Draw a card.",
            "X",
        );
        assert_eq!(n.rows, ["Choose one —", "• Target creature gets +2/+2 until end of turn.", "• Draw a card."]);
        assert_eq!(n.labels[1].as_ref().map(|l| l.after.as_str()), Some("• "));
        assert_eq!(n.labels[2], None);
        let n = roundtrip("I, II — Aerial Blast — Draw a card.\nIII — Domain — Draw a card.", "X");
        assert_eq!(n.rows, ["I, II — Draw a card.", "III — Draw a card."]);
        let n = roundtrip("Roll a d20.\n1—9 | Trapped! — You lose 3 life.\n10—19 | Draw a card.\n20 | Scry 1.", "X");
        assert_eq!(n.rows[1], "1—9 | You lose 3 life.");
        assert!(n.labels[2].is_none() && n.labels[3].is_none());
        // a keyword action alone names the mode (Secret Identity), as any other flavor word
        let n = roundtrip("Choose one —\n• Conceal — Draw a card.\n• Reveal — Draw a card.", "X");
        assert_eq!(n.rows, ["Choose one —", "• Draw a card.", "• Draw a card."]);
    }

    /// A label is a short title-cased phrase; sentence case and keyword dashes stay text, also
    /// after a marker.
    #[test]
    fn r207_2d_text_after_a_marker_is_not_a_label() {
        let text = "• Each opponent faces a villainous choice — lose 3 life.\n• Ward—Pay 3 life.\nIV — Target opponent faces a villainous choice — draw a card.";
        let n = roundtrip(text, "X");
        assert!(n.labels.iter().all(Option::is_none));
        assert_eq!(n.rows.join("\n"), text);
    }

    /// CR 614.12c: a bullet's anchor word is the one a "choose" row names; it is not a label.
    #[test]
    fn r614_12c_anchor_words_are_not_labels() {
        let text = "As ~ enters, choose Khans or Dragons.\n• Khans — At the beginning of your upkeep, draw a card.\n• Dragons — At the beginning of your upkeep, scry 1.";
        let n = roundtrip(text, "X");
        assert!(n.labels.iter().all(Option::is_none));
        assert_eq!(n.rows.join("\n"), text);
    }

    /// CR 702.148a: cleave removes the text in square brackets. The rows keep the words and drop the
    /// brackets; the cleaved rows drop the words; the brackets come back byte for byte, with the
    /// reminder text between them.
    #[test]
    fn r702_148a_cleave_brackets() {
        let text = "Cleave {4}{U}{U} (You may cast this spell for its cleave cost. If you do, remove the words in square brackets.)\nCounter target spell [that wasn't cast from its owner's hand].";
        let n = roundtrip_typed(text, "Wash Away", "Instant");
        assert_eq!(n.rows[1], "Counter target spell that wasn't cast from its owner's hand.");
        assert_eq!(n.cleaved().expect("cleave").rows[1], "Counter target spell.");
        assert_eq!(n.cleaved().expect("cleave").rows[0], "Cleave {4}{U}{U}");

        let text = "Cleave {1}{G}\nSearch your library for a [basic land] card, [reveal it,] put it into your hand, then shuffle.\nDraw three cards. [Your maximum hand size is reduced by three for the rest of the game.]";
        let n = roundtrip_typed(text, "Dig Up", "Sorcery");
        let c = n.cleaved().expect("cleave");
        assert_eq!(c.rows[1], "Search your library for a card, put it into your hand, then shuffle.");
        assert_eq!(c.rows[2], "Draw three cards.");
        assert_eq!(n.rows[1], "Search your library for a basic land card, reveal it, put it into your hand, then shuffle.");
    }

    /// Brackets mean something only to a spell with cleave: a loyalty cost in a quote keeps them.
    #[test]
    fn r702_148a_other_brackets_stay() {
        let text = "Enchanted planeswalker has \"[−12]: You get an emblem.\"";
        let n = roundtrip_typed(text, "Teferi's Talent", "Enchantment");
        assert!(n.cleaved().is_none());
        assert!(n.rows[0].contains("[−12]"));
    }

    #[test]
    fn reminder_without_space() {
        roundtrip("Investigate.(Create a Clue token.)", "Humble the Brute");
    }

    /// CR 201.5c with 205.3j: a planeswalker's first word is its type and its short name; it is
    /// the card unless a determiner or a type noun makes it the type.
    #[test]
    fn r201_5c_planeswalker_short_name() {
        let pw = "Legendary Planeswalker — Garruk";
        let n = roundtrip_typed("0: Garruk deals 3 damage to target creature.", "Garruk Relentless", pw);
        assert_eq!(n.rows[0], "0: ~ deals 3 damage to target creature.");
        let n = roundtrip_typed("Put a loyalty counter on each Garruk you control.", "Garruk, Cursed Huntsman", pw);
        assert_eq!(n.rows[0], "Put a loyalty counter on each Garruk you control.");
        let n = roundtrip_typed("Whenever you cast a Kaito planeswalker, draw.", "Kaito Shizuki", pw);
        assert_eq!(n.rows[0], "Whenever you cast a Kaito planeswalker, draw.");
        let n = roundtrip_typed("When Kiora enters, draw.", "Kiora of Fire and Ashes", "Legendary Creature — Merfolk");
        assert_eq!(n.rows[0], "When ~ enters, draw.");
        // a creature type is never a short name
        let n = roundtrip_typed("Other Goblin creatures get +1/+1.", "Goblin Chieftain", "Legendary Creature — Goblin");
        assert_eq!(n.rows[0], "Other Goblin creatures get +1/+1.");
    }

    /// CR 701, 205.3: a name that is only a game word is the word. Both sides: the card's real
    /// short names still read.
    #[test]
    fn r701_name_that_is_a_game_word() {
        let n = roundtrip_typed("Regenerate target creature.", "Regenerate", "Instant");
        assert_eq!(n.rows[0], "Regenerate target creature.");
        let n = roundtrip_typed("Destroy target land.", "Destroy the Evidence", "Sorcery");
        assert_eq!(n.rows[0], "Destroy target land.");
        let n = roundtrip_typed("Enchanted creature is a colorless Noggle.", "Noggle the Mind", "Enchantment — Aura");
        assert_eq!(n.rows[0], "Enchanted creature is a colorless Noggle.");
        let n = roundtrip_typed("Phage deals 2 damage to you.", "Phage the Untouchable", "Legendary Creature");
        assert_eq!(n.rows[0], "~ deals 2 damage to you.");
    }

    #[test]
    fn legendary_short_names() {
        let n = roundtrip_typed(
            "Remove a dream counter from Rasputin: Add {C}.",
            "Rasputin Dreamweaver",
            "Legendary Creature — Human Wizard",
        );
        assert_eq!(n.rows[0], "Remove a dream counter from ~: Add {C}.");
        let n = roundtrip_typed("Sacrifice King Darien: Draw a card.", "King Darien XLVIII", "Legendary Creature");
        assert_eq!(n.rows[0], "Sacrifice ~: Draw a card.");
        // part of another name, and not legendary
        let n = roundtrip_typed("Partner with Soulblade Renewer", "Soulblade Corrupter", "Legendary Creature");
        assert_eq!(n.rows[0], "Partner with Soulblade Renewer");
        let n = roundtrip_typed("Seek two nonland cards.", "Seek New Knowledge", "Sorcery");
        assert_eq!(n.rows[0], "Seek two nonland cards.");
    }
}
