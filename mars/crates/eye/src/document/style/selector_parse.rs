/** Owned selector grammar: compounds, four combinators, attributes and a bounded pseudo-class set. */
use super::constants::{SELECTOR_ARGUMENTS, SELECTOR_COMPOUNDS};
/// How a compound relates to the one on its left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Combinator {
    /// Whitespace: any ancestor.
    Descendant,
    /// `>`: the parent.
    Child,
    /// `+`: the previous element sibling.
    Next,
    /// `~`: any previous element sibling.
    Later,
}
/// Attribute comparison operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Operator {
    /// `[a]`
    Exists,
    /// `[a=v]`
    Equals,
    /// `[a^=v]`
    Prefix,
    /// `[a$=v]`
    Suffix,
    /// `[a*=v]`
    Substring,
    /// `[a~=v]`
    Word,
    /// `[a|=v]`
    Dash,
}
/// Pseudo-classes evaluated against the owned tree; dynamic ones never match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Pseudo {
    /// `:first-child`
    FirstChild,
    /// `:last-child`
    LastChild,
    /// `:only-child`
    OnlyChild,
    /// `:nth-child(an+b)` or, when `last`, `:nth-last-child(an+b)`.
    Nth { a: i64, b: i64, last: bool },
    /// `:nth-of-type(an+b)`, or `:nth-last-of-type` when `last`; first, last and only reduce to these.
    NthOfType { a: i64, b: i64, last: bool },
    /// `:only-of-type`
    OnlyOfType,
    /// `:root`
    Root,
    /// `:link` and `:any-link`: anchors with an href.
    Link,
    /// `:empty`
    Empty,
    /// `:checked`
    Checked,
    /// `:disabled`
    Disabled,
    /// `:enabled`
    Enabled,
    /// `:defined`: built-in elements, and custom elements a script upgraded.
    Defined,
    /// `:hover`, `:focus` and other user-state classes: the rule is kept and never matches.
    Never,
}
/// One simple selector inside a compound.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Simple {
    /// `#id`
    Id(String),
    /// `.class`
    Class(String),
    /// `[name op value i]`
    Attribute {
        name: String,
        operator: Operator,
        value: String,
        insensitive: bool,
    },
    /// Tree and state pseudo-classes.
    Pseudo(Pseudo),
    /// `:not(a, b)`: matches when no argument matches.
    Not(Vec<Compound>),
    /// `:is()` (counts specificity) or `:where()` (zero specificity).
    Is(Vec<Compound>, bool),
    /// `:has()`: relative selectors, each a leading combinator and a selector for the far element.
    Has(Vec<(Combinator, Selector)>),
}
/// A type selector (None is `*` or omitted) plus simple selectors.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Compound {
    /// Lowercase tag name, or None for any element.
    pub tag: Option<String>,
    /// Every simple selector must match.
    pub simples: Vec<Simple>,
}
/// A complex selector, leftmost compound first; `combinators[i]` joins compounds i and i+1.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Selector {
    /// Compounds in source order; the last is the subject.
    pub compounds: Vec<Compound>,
    /// One fewer than compounds.
    pub combinators: Vec<Combinator>,
}
/** A parse failure names the unsupported feature for diagnostics, never the author's text. */
pub(super) type Unsupported = &'static str;
/** Parses one complex selector, or names the first unsupported feature. */
pub(super) fn parse(source: &str) -> Result<Selector, Unsupported> {
    parse_in(source, false)
}
/** Parses one complex selector; inside `:has()` a nested `:has()` is refused, as CSS requires. */
fn parse_in(source: &str, in_has: bool) -> Result<Selector, Unsupported> {
    let mut cursor = Cursor::new(source.trim());
    cursor.in_has = in_has;
    let mut compounds = vec![cursor.compound()?];
    let mut combinators = Vec::new();
    while let Some(combinator) = cursor.combinator()? {
        combinators.push(combinator);
        compounds.push(cursor.compound()?);
        if compounds.len() > SELECTOR_COMPOUNDS {
            return Err("selector length");
        }
    }
    if !cursor.done() {
        return Err("selector syntax");
    }
    Ok(Selector {
        compounds,
        combinators,
    })
}
/** The subject compound without its `:has()` parts: what a budget-exhausted hiding rule masks. */
pub(super) fn has_free_subject(selector: &Selector) -> Compound {
    let mut subject = selector.compounds.last().cloned().unwrap_or_default();
    subject.simples.retain(|simple| !uses_has(simple));
    subject
}
/** Whether a simple selector is or contains `:has()`, so dropping it widens the match. */
fn uses_has(simple: &Simple) -> bool {
    match simple {
        Simple::Has(_) => true,
        Simple::Not(arguments) | Simple::Is(arguments, _) => arguments
            .iter()
            .any(|compound| compound.simples.iter().any(uses_has)),
        _ => false,
    }
}
/**
 * A superset of the subject a dropped selector could match, for hiding masks: the supported
 * simple selectors of its last compound. None means the selector styles a pseudo-element only.
 */
pub(super) fn subject(source: &str) -> Option<Compound> {
    let last = super::syntax::split_compounds(source.trim());
    let mut cursor = Cursor::new(last);
    let mut compound = Compound::default();
    if let Some(tag) = cursor.type_selector() {
        compound.tag = tag;
    }
    while !cursor.done() {
        if cursor.pseudo_element() {
            return None;
        }
        match cursor.simple() {
            // `:has()` is dropped from a mask subject: a superset must not depend on a budget.
            Ok(Some(Simple::Has(_))) => {}
            Ok(Some(simple)) if simple != Simple::Pseudo(Pseudo::Never) => {
                compound.simples.push(simple)
            }
            Ok(Some(_)) => {}
            _ => cursor.skip_simple(),
        }
    }
    Some(compound)
}
/** Byte cursor; every delimiter is ASCII so slicing stays on char boundaries. */
struct Cursor<'a> {
    /// Selector text.
    source: &'a str,
    /// Current byte offset.
    index: usize,
    /// Inside a `:has()` argument, where another `:has()` is invalid.
    in_has: bool,
}
impl<'a> Cursor<'a> {
    /** Starts at the beginning. */
    fn new(source: &'a str) -> Self {
        Self {
            source,
            index: 0,
            in_has: false,
        }
    }
    /** Remaining bytes. */
    fn rest(&self) -> &'a [u8] {
        &self.source.as_bytes()[self.index.min(self.source.len())..]
    }
    /** At the end of the input. */
    fn done(&self) -> bool {
        self.rest().is_empty()
    }
    /** The next byte. */
    fn peek(&self) -> Option<u8> {
        self.rest().first().copied()
    }
    /** Skips ASCII whitespace and reports whether any was skipped. */
    fn spaces(&mut self) -> bool {
        let start = self.index;
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.index += 1;
        }
        self.index > start
    }
    /** Whitespace and at most one of `> + ~` between compounds. */
    fn combinator(&mut self) -> Result<Option<Combinator>, Unsupported> {
        let spaced = self.spaces();
        let combinator = match self.peek() {
            None => return Ok(None),
            Some(b'>') => Combinator::Child,
            Some(b'+') => Combinator::Next,
            Some(b'~') => Combinator::Later,
            Some(b',' | b')') => return Ok(None),
            Some(_) if spaced => return Ok(Some(Combinator::Descendant)),
            Some(_) => return Err("selector syntax"),
        };
        self.index += 1;
        self.spaces();
        if self.done() {
            return Err("selector syntax");
        }
        Ok(Some(combinator))
    }
    /** `*`, a tag name, or nothing; Some(None) is the universal selector. */
    fn type_selector(&mut self) -> Option<Option<String>> {
        if self.peek() == Some(b'*') {
            self.index += 1;
            return Some(None);
        }
        if self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b >= 0x80)
        {
            return Some(Some(self.ident().to_ascii_lowercase()));
        }
        None
    }
    /** A compound must contain at least one selector. */
    fn compound(&mut self) -> Result<Compound, Unsupported> {
        let mut compound = Compound::default();
        let typed = self.type_selector();
        if self.peek() == Some(b'|') {
            return Err("namespace selector");
        }
        if let Some(tag) = typed.clone() {
            compound.tag = tag;
        }
        while let Some(simple) = self.simple()? {
            compound.simples.push(simple);
        }
        if typed.is_none() && compound.simples.is_empty() {
            return Err("selector syntax");
        }
        Ok(compound)
    }
    /** One `#id`, `.class`, `[attr]` or `:pseudo`; None at a compound boundary. */
    fn simple(&mut self) -> Result<Option<Simple>, Unsupported> {
        let simple = match self.peek() {
            Some(b'#') => {
                self.index += 1;
                Simple::Id(self.required_ident()?)
            }
            Some(b'.') => {
                self.index += 1;
                Simple::Class(self.required_ident()?)
            }
            Some(b'[') => {
                self.index += 1;
                self.attribute()?
            }
            Some(b':') => self.pseudo()?,
            _ => return Ok(None),
        };
        Ok(Some(simple))
    }
    /** Skips one unparseable simple selector, including any parenthesized argument. */
    fn skip_simple(&mut self) {
        self.index += 1;
        while let Some(b) = self.peek() {
            match b {
                b'#' | b'.' | b'[' | b':' => break,
                b'(' => self.index = self.closing(b')'),
                _ => self.index += 1,
            }
        }
    }
    /** Consumes a legacy or modern pseudo-element marker. */
    fn pseudo_element(&mut self) -> bool {
        let rest = self.rest();
        if rest.starts_with(b"::") {
            return true;
        }
        let legacy = [":before", ":after", ":first-line", ":first-letter"];
        legacy.iter().any(|name| {
            rest.len() >= name.len() && rest[..name.len()].eq_ignore_ascii_case(name.as_bytes())
        })
    }
    /** Identifier characters with CSS escapes decoded. */
    fn ident(&mut self) -> String {
        let mut out = String::new();
        while let Some(b) = self.peek() {
            if b == b'\\' {
                self.index += 1;
                self.escape(&mut out);
            } else if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80 {
                let ch = self.source[self.index..]
                    .chars()
                    .next()
                    .unwrap_or('\u{fffd}');
                out.push(ch);
                self.index += ch.len_utf8();
            } else {
                break;
            }
        }
        out
    }
    /** An identifier that must not be empty. */
    fn required_ident(&mut self) -> Result<String, Unsupported> {
        let ident = self.ident();
        if ident.is_empty() {
            Err("selector syntax")
        } else {
            Ok(ident)
        }
    }
    /** Hex escapes take up to six digits and one trailing space; others are literal. */
    fn escape(&mut self, out: &mut String) {
        let rest = self.rest();
        let digits = rest
            .iter()
            .take(6)
            .take_while(|b| b.is_ascii_hexdigit())
            .count();
        if digits > 0 {
            let text = &self.source[self.index..self.index + digits];
            let code = u32::from_str_radix(text, 16).unwrap_or(0xfffd);
            out.push(
                char::from_u32(code)
                    .filter(|c| *c != '\0')
                    .unwrap_or('\u{fffd}'),
            );
            self.index += digits;
            if self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
                self.index += 1;
            }
        } else if let Some(ch) = self.source[self.index.min(self.source.len())..]
            .chars()
            .next()
        {
            out.push(ch);
            self.index += ch.len_utf8();
        }
    }
    /** `[name]`, `[name op value]` with an optional `i` or `s` flag. */
    fn attribute(&mut self) -> Result<Simple, Unsupported> {
        self.spaces();
        let name = self.required_ident()?.to_ascii_lowercase();
        self.spaces();
        let operator = match self.rest() {
            [b']', ..] => Operator::Exists,
            [b'=', ..] => Operator::Equals,
            [b'^', b'=', ..] => Operator::Prefix,
            [b'$', b'=', ..] => Operator::Suffix,
            [b'*', b'=', ..] => Operator::Substring,
            [b'~', b'=', ..] => Operator::Word,
            [b'|', b'=', ..] => Operator::Dash,
            _ => return Err("attribute selector"),
        };
        self.index += match operator {
            Operator::Exists => 0,
            Operator::Equals => 1,
            _ => 2,
        };
        self.spaces();
        let value = if operator == Operator::Exists {
            String::new()
        } else {
            self.attribute_value()?
        };
        self.spaces();
        let insensitive = match self.peek() {
            Some(b'i' | b'I') => true,
            Some(b's' | b'S') => false,
            _ => false,
        };
        if self.peek().is_some_and(|b| b != b']') {
            self.index += 1;
            self.spaces();
        }
        if self.peek() != Some(b']') {
            return Err("attribute selector");
        }
        self.index += 1;
        Ok(Simple::Attribute {
            name,
            operator,
            value,
            insensitive,
        })
    }
    /** A quoted string or a bare identifier. */
    fn attribute_value(&mut self) -> Result<String, Unsupported> {
        let Some(quote @ (b'"' | b'\'')) = self.peek() else {
            return Ok(self.ident());
        };
        self.index += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("attribute selector"),
                Some(b) if b == quote => break,
                Some(b'\\') => {
                    self.index += 1;
                    self.escape(&mut out);
                }
                Some(_) => {
                    let ch = self.source[self.index..]
                        .chars()
                        .next()
                        .unwrap_or('\u{fffd}');
                    out.push(ch);
                    self.index += ch.len_utf8();
                }
            }
        }
        self.index += 1;
        Ok(out)
    }
    /** Index just past the matching close of a parenthesis or bracket at the cursor. */
    fn closing(&self, close: u8) -> usize {
        let open = self.peek().unwrap_or(b'(');
        let mut depth = 0usize;
        let bytes = self.source.as_bytes();
        let mut index = self.index;
        while index < bytes.len() {
            if bytes[index] == open {
                depth += 1;
            } else if bytes[index] == close || (open == b'[' && bytes[index] == b']') {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index + 1;
                }
            }
            index += 1;
        }
        bytes.len()
    }
    /** Pseudo-classes; pseudo-elements and unknown names are unsupported. */
    fn pseudo(&mut self) -> Result<Simple, Unsupported> {
        if self.pseudo_element() {
            return Err("pseudo-element");
        }
        self.index += 1;
        let name = self.required_ident()?.to_ascii_lowercase();
        if self.peek() == Some(b'(') {
            return self.functional(&name);
        }
        let pseudo = match name.as_str() {
            "first-child" => Pseudo::FirstChild,
            "last-child" => Pseudo::LastChild,
            "only-child" => Pseudo::OnlyChild,
            "first-of-type" => Pseudo::NthOfType {
                a: 0,
                b: 1,
                last: false,
            },
            "last-of-type" => Pseudo::NthOfType {
                a: 0,
                b: 1,
                last: true,
            },
            "only-of-type" => Pseudo::OnlyOfType,
            "root" => Pseudo::Root,
            "link" | "any-link" => Pseudo::Link,
            "empty" => Pseudo::Empty,
            "checked" => Pseudo::Checked,
            "disabled" => Pseudo::Disabled,
            "enabled" => Pseudo::Enabled,
            "defined" => Pseudo::Defined,
            "hover" | "active" | "focus" | "visited" | "focus-visible" | "focus-within"
            | "target" | "placeholder-shown" | "autofill" | "-webkit-autofill" => Pseudo::Never,
            _ => return Err(pseudo_name(&name)),
        };
        Ok(Simple::Pseudo(pseudo))
    }
    /** `:nth-child()`, `:nth-last-child()`, `:not()`, `:is()` and `:where()`. */
    fn functional(&mut self, name: &str) -> Result<Simple, Unsupported> {
        let end = self.closing(b')');
        let inner = self.source.get(self.index + 1..end.saturating_sub(1));
        let inner = inner.ok_or("selector syntax")?;
        self.index = end;
        match name {
            "nth-child" | "nth-last-child" => {
                let (a, b) = nth(inner).ok_or("nth-child")?;
                Ok(Simple::Pseudo(Pseudo::Nth {
                    a,
                    b,
                    last: name == "nth-last-child",
                }))
            }
            "nth-of-type" | "nth-last-of-type" => {
                let (a, b) = nth(inner).ok_or(":nth-of-type")?;
                Ok(Simple::Pseudo(Pseudo::NthOfType {
                    a,
                    b,
                    last: name == "nth-last-of-type",
                }))
            }
            "not" => Ok(Simple::Not(compounds(inner, self.in_has)?)),
            "is" | "matches" | "-webkit-any" => {
                Ok(Simple::Is(compounds(inner, self.in_has)?, false))
            }
            "where" => Ok(Simple::Is(compounds(inner, self.in_has)?, true)),
            // The name is escape-decoded and lowercased, so `:HAS(` and `:h\61s(` land here too.
            "has" if self.in_has => Err(":has"),
            "has" => Ok(Simple::Has(relative(inner)?)),
            _ => Err(pseudo_name(name)),
        }
    }
}
/** `:has()` arguments: an optional leading `>`, `+` or `~` (descendant when absent), then a selector. */
fn relative(source: &str) -> Result<Vec<(Combinator, Selector)>, Unsupported> {
    let parts = super::syntax::split_top(source, b',');
    if parts.is_empty() || parts.len() > SELECTOR_ARGUMENTS {
        return Err(":has");
    }
    parts
        .into_iter()
        .map(|part| {
            let part = part.trim();
            let (combinator, rest) = match part.as_bytes().first() {
                Some(b'>') => (Combinator::Child, &part[1..]),
                Some(b'+') => (Combinator::Next, &part[1..]),
                Some(b'~') => (Combinator::Later, &part[1..]),
                _ => (Combinator::Descendant, part),
            };
            Ok((combinator, parse_in(rest, true)?))
        })
        .collect()
}
/** A comma list of compounds; complex arguments are unsupported. */
fn compounds(source: &str, in_has: bool) -> Result<Vec<Compound>, Unsupported> {
    let parts = super::syntax::split_top(source, b',');
    if parts.is_empty() || parts.len() > SELECTOR_ARGUMENTS {
        return Err("selector arguments");
    }
    parts
        .into_iter()
        .map(|part| {
            let mut cursor = Cursor::new(part.trim());
            cursor.in_has = in_has;
            let compound = cursor.compound()?;
            if cursor.done() {
                Ok(compound)
            } else {
                Err("complex selector argument")
            }
        })
        .collect()
}
/** `odd`, `even`, `b`, `an`, `an+b` and `an-b`; an `of S` clause is unsupported. */
fn nth(source: &str) -> Option<(i64, i64)> {
    let text: String = source
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    let bounded = |value: i64| (value.abs() <= i64::from(i32::MAX)).then_some(value);
    match text.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let Some((a, b)) = text.split_once('n') else {
        return Some((0, bounded(text.parse().ok()?)?));
    };
    let a = match a {
        "" | "+" => 1,
        "-" => -1,
        a => bounded(a.parse().ok()?)?,
    };
    let b = if b.is_empty() {
        0
    } else {
        bounded(b.parse().ok()?)?
    };
    Some((a, b))
}
/** Known unsupported pseudo-classes are named; anything else is reported generically. */
fn pseudo_name(name: &str) -> Unsupported {
    [
        ":has",
        ":lang",
        ":dir",
        ":host",
        ":required",
        ":invalid",
        ":valid",
        ":read-only",
        ":read-write",
    ]
    .into_iter()
    .find(|known| known[1..] == *name)
    .unwrap_or("pseudo-class")
}
/** Specificity as ids, classes (attributes and pseudo-classes) and types in one scalar. */
pub(super) fn specificity(selector: &Selector) -> usize {
    let (ids, classes, types) = selector
        .compounds
        .iter()
        .map(compound_specificity)
        .fold((0, 0, 0), |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2));
    ids * 1_000_000 + classes.min(999) * 1000 + types.min(999)
}
/** One compound's specificity triple; `:is`/`:not` count their most specific argument. */
fn compound_specificity(compound: &Compound) -> (usize, usize, usize) {
    let mut triple = (0, 0, usize::from(compound.tag.is_some()));
    for simple in &compound.simples {
        let add = match simple {
            Simple::Id(_) => (1, 0, 0),
            Simple::Is(_, true) => (0, 0, 0),
            Simple::Not(arguments) | Simple::Is(arguments, false) => arguments
                .iter()
                .map(compound_specificity)
                .max()
                .unwrap_or_default(),
            Simple::Has(arguments) => arguments
                .iter()
                .map(|(_, selector)| {
                    let n = specificity(selector);
                    (n / 1_000_000, n / 1000 % 1000, n % 1000)
                })
                .max()
                .unwrap_or_default(),
            _ => (0, 1, 0),
        };
        triple = (triple.0 + add.0, triple.1 + add.1, triple.2 + add.2);
    }
    triple
}
