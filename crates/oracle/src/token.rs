//! Normalized rows → tokens, losslessly: [`detokenize`] rebuilds the row from the tokens alone.
//!
//! The lexer only cuts the text into pieces; it never decides meaning. "'s" is a separate
//! [`Kind::Clitic`] whether it is a possessive ("opponent's") or a contraction ("it's"), and "X"
//! is a word: the grammar reads both in context.

use crate::normalize::{GRANTING_SELF, SELF};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Letters and digits with inner hyphens and apostrophes: "creature", "non-Human",
    /// "can't", "d20", "ninety-nine", "X".
    Word(String),
    /// A run of digits, with thousands separators as printed ("1,000"). `text` keeps the
    /// spelling; `value` is what it means.
    Number { value: u32, text: String },
    /// "'s" or a bare "'" after a word ("opponents'").
    Clitic(String),
    /// The inside of a `{…}` symbol: "T", "2", "W/U", "G/P", "E", "CHAOS".
    Symbol(String),
    /// Any other single character: `, . : ; ! ? — − - + / | [ ] • = &` ….
    Punct(char),
    /// [`SELF`].
    SelfRef,
    /// [`GRANTING_SELF`].
    GrantingSelf,
    /// A quoted ability (or name), lexed recursively. `mark` is `"` or, nested inside one, `'`.
    /// `closed` is false when the row ends before the closing mark.
    Quote { mark: char, body: Vec<Token>, closed: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: Kind,
    /// Spaces between the previous token (or the row start) and this one.
    pub space_before: u8,
}

impl Token {
    pub fn word(&self) -> Option<&str> {
        match &self.kind {
            Kind::Word(w) => Some(w),
            _ => None,
        }
    }
}

pub fn tokenize(row: &str) -> Vec<Token> {
    let chars: Vec<char> = row.chars().collect();
    let mut i = 0;
    let (tokens, _) = lex(&chars, &mut i, None);
    tokens
}

/// Lex until `close` (or the end). Returns the tokens and whether `close` was found.
fn lex(c: &[char], i: &mut usize, close: Option<char>) -> (Vec<Token>, bool) {
    let mut out: Vec<Token> = Vec::new();
    loop {
        let mut spaces = 0u8;
        while *i < c.len() && c[*i] == ' ' {
            spaces = spaces.saturating_add(1);
            *i += 1;
        }
        if *i >= c.len() {
            if spaces > 0 {
                // trailing spaces: kept as a zero-width marker so detokenize stays exact
                out.push(Token { kind: Kind::Punct('\u{0}'), space_before: spaces });
            }
            return (out, false);
        }
        let ch = c[*i];
        let after_word = spaces == 0
            && out.last().is_some_and(|t| {
                matches!(t.kind, Kind::Word(_) | Kind::SelfRef | Kind::GrantingSelf | Kind::Number { .. })
            });
        // after a plural word, "'" before a space is a possessive ("opponents' hands"); anywhere
        // else inside a nested quotation it closes it ("… any target'\"")
        let plural_possessive = after_word
            && out.last().and_then(Token::word).is_some_and(|w| w.ends_with('s'))
            && c.get(*i + 1) == Some(&' ');
        if Some(ch) == close {
            // a `'` right after a word with no nested quote pending is a clitic, handled below;
            // here we only get a matching close mark
            let is_clitic =
                ch == '\'' && after_word && (c.get(*i + 1) == Some(&'s') && starts_clitic(c, *i) || plural_possessive);
            if !is_clitic {
                if spaces > 0 {
                    out.push(Token { kind: Kind::Punct('\u{0}'), space_before: spaces });
                }
                *i += 1;
                return (out, true);
            }
        }
        let start = *i;
        let kind = match ch {
            SELF => {
                *i += 1;
                Kind::SelfRef
            }
            GRANTING_SELF => {
                *i += 1;
                Kind::GrantingSelf
            }
            '"' => {
                *i += 1;
                let (body, closed) = lex(c, i, Some('"'));
                Kind::Quote { mark: '"', body, closed }
            }
            // a single quote opens a nested quotation only inside a double-quoted one, at a token
            // start, before a non-space
            '\'' if close == Some('"') && !after_word && c.get(*i + 1).is_some_and(|n| *n != ' ') => {
                *i += 1;
                let (body, closed) = lex(c, i, Some('\''));
                Kind::Quote { mark: '\'', body, closed }
            }
            '\'' if after_word && starts_clitic(c, *i) => {
                *i += 1;
                if c.get(*i) == Some(&'s') {
                    *i += 1;
                    Kind::Clitic("'s".into())
                } else {
                    Kind::Clitic("'".into())
                }
            }
            '{' => match c[*i..].iter().position(|&x| x == '}') {
                Some(len) => {
                    let inner: String = c[*i + 1..*i + len].iter().collect();
                    *i += len + 1;
                    Kind::Symbol(inner)
                }
                None => {
                    *i += 1;
                    Kind::Punct('{')
                }
            },
            d if d.is_ascii_digit() => {
                while *i < c.len() && c[*i].is_ascii_digit() {
                    *i += 1;
                }
                // thousands groups: ",ddd" not followed by another digit
                while c.get(*i) == Some(&',')
                    && c.len() >= *i + 4
                    && c[*i + 1..*i + 4].iter().all(char::is_ascii_digit)
                    && !c.get(*i + 4).is_some_and(char::is_ascii_digit)
                {
                    *i += 4;
                }
                if *i < c.len() && c[*i].is_alphabetic() {
                    *i = start;
                    Kind::Word(word(c, i))
                } else {
                    let text: String = c[start..*i].iter().collect();
                    match text.replace(',', "").parse() {
                        Ok(value) => Kind::Number { value, text },
                        Err(_) => Kind::Word(text),
                    }
                }
            }
            a if a.is_alphabetic() => Kind::Word(word(c, i)),
            p => {
                *i += 1;
                Kind::Punct(p)
            }
        };
        out.push(Token { kind, space_before: spaces });
    }
}

/// Is the `'` at `i` the start of a clitic ("'s" not followed by a letter, or a bare "'" not
/// followed by a letter)?
fn starts_clitic(c: &[char], i: usize) -> bool {
    match c.get(i + 1) {
        Some('s') => !c.get(i + 2).is_some_and(|n| n.is_alphanumeric()),
        Some(n) => !n.is_alphanumeric(),
        None => true,
    }
}

/// Letters and digits, with a hyphen or apostrophe allowed between two of them ("non-Human",
/// "can't", "C'tan"); "'s" at the end is left for the clitic.
fn word(c: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < c.len() {
        let ch = c[*i];
        let joiner = (ch == '-' || ch == '\'')
            && c.get(*i + 1).is_some_and(|n| n.is_alphanumeric())
            && !(ch == '\'' && starts_clitic(c, *i));
        if !(ch.is_alphanumeric() || joiner) {
            break;
        }
        *i += 1;
    }
    c[start..*i].iter().collect()
}

/// The row these tokens came from.
pub fn detokenize(tokens: &[Token]) -> String {
    let mut s = String::new();
    write(tokens, &mut s);
    s
}

fn write(tokens: &[Token], s: &mut String) {
    for t in tokens {
        s.extend(std::iter::repeat_n(' ', t.space_before as usize));
        match &t.kind {
            Kind::Word(w) | Kind::Clitic(w) => s.push_str(w),
            Kind::Number { text, .. } => s.push_str(text),
            Kind::Symbol(x) => {
                s.push('{');
                s.push_str(x);
                s.push('}');
            }
            Kind::Punct('\u{0}') => {}
            Kind::Punct(p) => s.push(*p),
            Kind::SelfRef => s.push(SELF),
            Kind::GrantingSelf => s.push(GRANTING_SELF),
            Kind::Quote { mark, body, closed } => {
                s.push(*mark);
                write(body, s);
                if *closed {
                    s.push(*mark);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(row: &str) -> Vec<Kind> {
        let t = tokenize(row);
        assert_eq!(detokenize(&t), row);
        t.into_iter().map(|t| t.kind).collect()
    }

    fn num(n: u32) -> Kind {
        Kind::Number { value: n, text: n.to_string() }
    }

    fn w(s: &str) -> Kind {
        Kind::Word(s.into())
    }

    #[test]
    fn words_numbers_symbols() {
        assert_eq!(
            kinds("{2}{U}, {T}: Draw 2 cards."),
            vec![
                Kind::Symbol("2".into()),
                Kind::Symbol("U".into()),
                Kind::Punct(','),
                Kind::Symbol("T".into()),
                Kind::Punct(':'),
                w("Draw"),
                num(2),
                w("cards"),
                Kind::Punct('.'),
            ]
        );
    }

    #[test]
    fn clitics_and_contractions() {
        assert_eq!(
            kinds("~'s owner can't target opponents' non-Human creatures."),
            vec![
                Kind::SelfRef,
                Kind::Clitic("'s".into()),
                w("owner"),
                w("can't"),
                w("target"),
                w("opponents"),
                Kind::Clitic("'".into()),
                w("non-Human"),
                w("creatures"),
                Kind::Punct('.'),
            ]
        );
    }

    #[test]
    fn pt_and_loyalty() {
        assert_eq!(
            kinds("−2: Target creature gets +1/-1."),
            vec![
                Kind::Punct('−'),
                num(2),
                Kind::Punct(':'),
                w("Target"),
                w("creature"),
                w("gets"),
                Kind::Punct('+'),
                num(1),
                Kind::Punct('/'),
                Kind::Punct('-'),
                num(1),
                Kind::Punct('.'),
            ]
        );
    }

    #[test]
    fn quotes_nest() {
        let k = kinds("It has \"When ~ dies, create a token with 'Flying.'\"");
        let Kind::Quote { mark: '"', body, closed: true } = &k[2] else { panic!("{k:?}") };
        assert!(body.iter().any(|t| matches!(&t.kind, Kind::Quote { mark: '\'', closed: true, .. })));
    }

    #[test]
    fn thousands_and_nested_close_after_word() {
        assert_eq!(kinds("loses 1,000 life, 2, 3")[1], Kind::Number { value: 1000, text: "1,000".into() });
        let k = kinds("with \"Equipped creature has '{1}, {T}: ~ deals 2 damage to any target'\" and equip {1}.");
        let Kind::Quote { body, closed: true, .. } = &k[1] else { panic!("{k:?}") };
        assert!(matches!(body.last().map(|t| &t.kind), Some(Kind::Quote { mark: '\'', closed: true, .. })));
    }

    #[test]
    fn possessive_inside_quote() {
        let k = kinds("\"Return it to its owners' hands.\"");
        let Kind::Quote { body, closed: true, .. } = &k[0] else { panic!("{k:?}") };
        assert!(body.iter().any(|t| t.kind == Kind::Clitic("'".into())));
    }
}
