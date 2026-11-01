/** Owned selector grammar: compounds, four combinators, attributes and a bounded pseudo-class set. */
use super::constants::{SELECTOR_ARGUMENTS, SELECTOR_COMPOUNDS};
use super::selector_names::{chrome_pseudo_element, known_pseudo_element, never_pseudo_class};
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
    /// `:dir(ltr)` (false) or `:dir(rtl)` (true): the element's directionality.
    Dir(bool),
    /// `:placeholder-shown`: a text control showing its placeholder.
    PlaceholderShown,
    /// `:read-write` (true) or `:read-only` (false).
    ReadWrite(bool),
    /// `:required` (true) or `:optional` (false), on form controls only.
    Required(bool),
    /// `:valid` (true) or `:invalid` (false), on form controls only.
    Valid(bool),
    /// `:indeterminate`: a `<progress>` without a value.
    Indeterminate,
    /// `:default`: a checkbox, radio or option selected by its markup.
    Default,
    /// `:open`: a `<details>` or `<dialog>` with `open`.
    Open,
    /// `:focus`: the element a script's `focus()` left focused.
    Focus,
    /// `:focus-within`: the focused element or one of its flat-tree ancestors.
    FocusWithin,
    /// `:focus-visible`: a focused text-entry element, the case a script's focus and typing reach.
    FocusVisible,
    /// `:hover` and other user-state classes: the rule is kept and never matches.
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
    /// `:not(a, b)`: matches when no argument matches. Arguments are complex selectors whose
    /// subject is the element itself, as in Blink's SelectorChecker.
    Not(Vec<Selector>),
    /// `:is()` (counts specificity) or `:where()` (zero specificity), over complex selectors.
    Is(Vec<Selector>, bool),
    /// `:has()`: relative selectors, each a leading combinator and a selector for the far element.
    Has(Vec<(Combinator, Selector)>),
    /// `:lang()`: language ranges, lowercased, matched by RFC 4647 extended filtering.
    Lang(Vec<String>),
    /// `:nth-child(an+b of S)` or `:nth-last-child(an+b of S)`: position among siblings matching S.
    NthOf {
        a: i64,
        b: i64,
        last: bool,
        of: Vec<Selector>,
    },
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
/**
 * The failure of a selector Blink itself rejects (an unknown or other-engine pseudo, a pseudo-element
 * out of place). Blink drops such a rule silently, so Eye neither logs it nor masks anything for it.
 */
pub(super) const INVALID: Unsupported = "invalid selector";
/** Parses one complex selector, or names the first unsupported feature. */
pub(super) fn parse(source: &str) -> Result<Selector, Unsupported> {
    parse_in(source, false)
}
/** Parses one complex selector; inside `:has()` a nested `:has()` is refused, as CSS requires. */
fn parse_in(source: &str, in_has: bool) -> Result<Selector, Unsupported> {
    parse_nested(source, in_has, 0)
}
/// Selector functions nest at most this deep (`:is(:not(:is(…)))`), bounding parse recursion.
const SELECTOR_NESTING: usize = 32;
/** Parses one complex selector `depth` selector functions deep. */
fn parse_nested(source: &str, in_has: bool, depth: usize) -> Result<Selector, Unsupported> {
    if depth > SELECTOR_NESTING {
        return Err("selector length");
    }
    let mut cursor = Cursor::new(source.trim());
    cursor.in_has = in_has;
    cursor.depth = depth;
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
        Simple::Not(arguments) | Simple::Is(arguments, _) => arguments.iter().any(|selector| {
            selector
                .compounds
                .iter()
                .any(|compound| compound.simples.iter().any(uses_has))
        }),
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
        let start = cursor.index;
        match cursor.simple() {
            // `:has()` is dropped from a mask subject: a superset must not depend on a budget.
            Ok(Some(Simple::Has(_))) => {}
            Ok(Some(simple)) if simple != Simple::Pseudo(Pseudo::Never) => {
                compound.simples.push(simple)
            }
            Ok(Some(_)) => {}
            _ => {
                // A failed simple may have consumed part of the text; skip it from its start, so
                // a following `::pseudo-element` is still seen (Google's view-transition rules).
                cursor.index = start;
                cursor.skip_simple();
            }
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
    /// Selector functions enclosing this selector.
    depth: usize,
    /// A pseudo-element was read in the current compound, so only pseudo-classes may follow it.
    element: bool,
}
impl<'a> Cursor<'a> {
    /** Starts at the beginning. */
    fn new(source: &'a str) -> Self {
        Self {
            source,
            index: 0,
            in_has: false,
            depth: 0,
            element: false,
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
        // Blink: a pseudo-element is the last compound, so a combinator after it is invalid.
        if self.element {
            return Err(INVALID);
        }
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
    /**
     * A namespace-prefixed type selector, as Blink resolves it with no `@namespace` rules:
     * `*|E` matches E in any namespace, `|E` only elements without one (never an HTML, SVG or
     * MathML element), and a named prefix is undeclared, which makes the selector invalid.
     */
    fn namespaced(
        &mut self,
        prefix: Option<Option<String>>,
        compound: &mut Compound,
    ) -> Result<Option<Option<String>>, Unsupported> {
        self.index += 1;
        let local = self.type_selector().ok_or("namespace selector")?;
        match prefix {
            Some(None) => {}
            None => compound.simples.push(Simple::Pseudo(Pseudo::Never)),
            Some(Some(_)) => return Err("namespace selector"),
        }
        Ok(Some(local))
    }
    /** A compound must contain at least one selector. */
    fn compound(&mut self) -> Result<Compound, Unsupported> {
        self.element = false;
        let mut compound = Compound::default();
        let mut typed = self.type_selector();
        if self.peek() == Some(b'|') {
            typed = self.namespaced(typed, &mut compound)?;
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
        // After a pseudo-element only pseudo-classes may follow in its compound (`::before.a` is invalid).
        if self.element && matches!(self.peek(), Some(b'#' | b'.' | b'[')) {
            return Err(INVALID);
        }
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
        // `[|name]` (no namespace) and `[*|name]` (any): an HTML attribute has none, so both
        // match as `[name]`; the name must follow the bar with no space between.
        match self.rest() {
            [b'|', next, ..] if *next != b'=' => self.index += 1,
            [b'*', b'|', ..] => self.index += 2,
            _ => {}
        }
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
    /**
     * A pseudo-element. Blink accepts the known ones anywhere they end a complex selector and
     * matches none of them against an element, so Eye keeps the rule and it never applies. Unknown
     * and other-engine names, or one out of place, make the selector invalid.
     */
    fn pseudo_element_simple(&mut self) -> Result<Simple, Unsupported> {
        if self.depth > 0 || self.in_has || self.element {
            return Err(INVALID);
        }
        let legacy = self.rest().starts_with(b":") && !self.rest().starts_with(b"::");
        self.index += if legacy { 1 } else { 2 };
        let name = self.ident().to_ascii_lowercase();
        if self.peek() == Some(b'(') {
            self.index = self.closing(b')');
        }
        self.element = true;
        if !known_pseudo_element(&name) {
            return Err(INVALID);
        }
        // Any pseudo-element that can reach a page's own text or elements stays unsupported, so a
        // hiding rule on it masks rather than matching nothing.
        if !chrome_pseudo_element(&name) {
            return Err("pseudo-element");
        }
        Ok(Simple::Pseudo(Pseudo::Never))
    }
    /** Pseudo-classes; pseudo-elements go to `pseudo_element_simple`, unknown names are unsupported. */
    fn pseudo(&mut self) -> Result<Simple, Unsupported> {
        if self.pseudo_element() {
            return self.pseudo_element_simple();
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
            "-webkit-any-link" => Pseudo::Link,
            "scope" => Pseudo::Root,
            "placeholder-shown" => Pseudo::PlaceholderShown,
            "read-write" => Pseudo::ReadWrite(true),
            "read-only" => Pseudo::ReadWrite(false),
            "required" => Pseudo::Required(true),
            "optional" => Pseudo::Required(false),
            "valid" => Pseudo::Valid(true),
            "invalid" => Pseudo::Valid(false),
            "indeterminate" => Pseudo::Indeterminate,
            "default" => Pseudo::Default,
            "open" => Pseudo::Open,
            "focus" => Pseudo::Focus,
            "focus-within" => Pseudo::FocusWithin,
            "focus-visible" => Pseudo::FocusVisible,
            // States a page reaches only through a person or a script call Eye has not made.
            "hover"
            | "active"
            | "visited"
            | "target"
            | "autofill"
            | "-webkit-autofill"
            | "popover-open"
            | "modal"
            | "fullscreen"
            | "-webkit-full-screen"
            | "picture-in-picture"
            | "user-invalid"
            | "user-valid"
            | "target-within"
            | "playing"
            | "paused"
            | "seeking"
            | "buffering"
            | "stalled"
            | "muted"
            | "volume-locked"
            | "closed" => Pseudo::Never,
            _ if never_pseudo_class(&name) => Pseudo::Never,
            _ => return Err(refusal(&name)),
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
                let last = name == "nth-last-child";
                if let Some((formula, of)) = split_of(inner) {
                    let (a, b) = nth(formula).ok_or("nth-child")?;
                    let of = arguments(of, self.in_has, self.depth, false)?;
                    if of.iter().any(nests_nth_of) {
                        return Err(":nth-child(of S) inside S");
                    }
                    return Ok(Simple::NthOf { a, b, last, of });
                }
                let (a, b) = nth(inner).ok_or("nth-child")?;
                Ok(Simple::Pseudo(Pseudo::Nth { a, b, last }))
            }
            "lang" => lang(inner).map(Simple::Lang).ok_or(":lang"),
            "dir" => match inner.trim().to_ascii_lowercase().as_str() {
                "ltr" => Ok(Simple::Pseudo(Pseudo::Dir(false))),
                "rtl" => Ok(Simple::Pseudo(Pseudo::Dir(true))),
                _ => Err(":dir"),
            },
            "nth-of-type" | "nth-last-of-type" => {
                let (a, b) = nth(inner).ok_or(":nth-of-type")?;
                Ok(Simple::Pseudo(Pseudo::NthOfType {
                    a,
                    b,
                    last: name == "nth-last-of-type",
                }))
            }
            "not" => Ok(Simple::Not(arguments(
                inner,
                self.in_has,
                self.depth,
                false,
            )?)),
            "is" | "matches" | "-webkit-any" => Ok(Simple::Is(
                arguments(inner, self.in_has, self.depth, true)?,
                false,
            )),
            "where" => Ok(Simple::Is(
                arguments(inner, self.in_has, self.depth, true)?,
                true,
            )),
            // The name is escape-decoded and lowercased, so `:HAS(` and `:h\61s(` land here too.
            "has" if self.in_has => Err(":has"),
            "has" => Ok(Simple::Has(relative(inner)?)),
            _ => Err(refusal(name)),
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
/** Another engine's prefixed name (`-moz-`, `-ms-`, `-o-`), which Blink never accepts. */
fn foreign_name(name: &str) -> bool {
    ["-moz-", "-ms-", "-o-"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}
/** The refusal for a pseudo-class Eye does not know: invalid when Blink never accepts it. */
fn refusal(name: &str) -> Unsupported {
    if foreign_name(name) {
        INVALID
    } else {
        pseudo_name(name)
    }
}
/**
 * Whether a selector holds `:nth-child(of S)` anywhere inside it. Each such argument scans every
 * sibling of every candidate, so nesting one in another multiplies the work by the sibling count
 * per level; a nested one is refused (the rule falls back to its mask) to keep matching linear.
 */
fn nests_nth_of(selector: &Selector) -> bool {
    selector
        .compounds
        .iter()
        .flat_map(|c| &c.simples)
        .any(|simple| match simple {
            Simple::NthOf { .. } => true,
            Simple::Not(list) | Simple::Is(list, _) => list.iter().any(nests_nth_of),
            Simple::Has(list) => list.iter().any(|(_, s)| nests_nth_of(s)),
            _ => false,
        })
}
/**
 * A comma list of complex selectors, as Selectors 4 and Blink parse `:is()`, `:where()` and
 * `:not()`. `:is()` and `:where()` take a forgiving list: an invalid argument is dropped and the
 * rest still match (an empty list matches nothing); in `:not()` it invalidates the selector.
 * Other arguments Eye cannot parse fail the rule, so masks stay fail-closed.
 */
fn arguments(
    source: &str,
    in_has: bool,
    depth: usize,
    forgiving: bool,
) -> Result<Vec<Selector>, Unsupported> {
    let parts = super::syntax::split_top(source, b',');
    if (parts.is_empty() && !forgiving) || parts.len() > SELECTOR_ARGUMENTS {
        return Err("selector arguments");
    }
    let mut out = Vec::new();
    for part in parts {
        match parse_nested(part, in_has, depth + 1) {
            Ok(selector) => out.push(selector),
            // Blink drops an invalid argument from a forgiving list and keeps the rest. Anything
            // Eye cannot parse fails the rule, so a hiding rule falls back to its mask.
            Err(INVALID) if forgiving => {}
            Err(feature) => return Err(feature),
        }
    }
    Ok(out)
}
/**
 * `An+B of S` split at the `of` keyword, as Blink's tokenizer sees it: `of` is its own ident
 * token, so whitespace (or a comment) precedes it and no ident character follows it. `3 of.x`
 * and `3 of/* c */x` split; `1of x` (a dimension) and `n of` with nothing after do not.
 */
fn split_of(source: &str) -> Option<(&str, &str)> {
    let bytes = source.as_bytes();
    let ident =
        |b: u8| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80 || b == b'\\';
    (1..bytes.len().saturating_sub(1)).find_map(|at| {
        // Checked on bytes, and the ASCII "of" first: only then are `at` and `at + 2` char
        // boundaries, so slicing a selector holding multibyte text never panics.
        if !bytes[at..at + 2].eq_ignore_ascii_case(b"of") {
            return None;
        }
        let before = bytes[at - 1].is_ascii_whitespace() || bytes[..at].ends_with(b"*/");
        let after = bytes.get(at + 2).is_some_and(|&b| !ident(b));
        (before && after).then(|| (&source[..at], &source[at + 2..]))
    })
}
/** `:lang()` ranges: identifiers or strings, comma-separated, lowercased; None when empty. */
fn lang(source: &str) -> Option<Vec<String>> {
    let ranges: Vec<String> = super::syntax::split_top(source, b',')
        .into_iter()
        .map(|range| {
            range
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .replace('\\', "")
                .to_ascii_lowercase()
        })
        .collect();
    (!ranges.is_empty() && ranges.len() <= SELECTOR_ARGUMENTS).then_some(ranges)
}
/** `odd`, `even`, `b`, `an`, `an+b` and `an-b`. */
fn nth(source: &str) -> Option<(i64, i64)> {
    let text = source.trim().to_ascii_lowercase();
    let bounded = |value: i64| (value.abs() <= i64::from(i32::MAX)).then_some(value);
    match text.as_str() {
        "odd" => return Some((2, 1)),
        "even" => return Some((2, 0)),
        _ => {}
    }
    let Some((a, b)) = text.split_once('n') else {
        let b = unspaced(&text).then(|| text.parse().ok()).flatten();
        return Some((0, bounded(b?)?));
    };
    let a = match a {
        "" | "+" => 1,
        "-" => -1,
        a if unspaced(a) => bounded(a.parse().ok()?)?,
        _ => return None,
    };
    Some((a, bounded(nth_offset(b.trim_start())?)?))
}
/** Whether An+B text holds no whitespace: `3 n`, `+ n` and `+ 5` are invalid (css-syntax
 * 3 section 6), so a rule using one is dropped. */
fn unspaced(text: &str) -> bool {
    !text.contains(char::is_whitespace)
}
/** The B of An+B after the `n`: nothing, or a sign then an unsigned integer, with whitespace
 * allowed only around the sign (`n + 1`, `n +1`, `n- 1`, never `n + -1`). */
fn nth_offset(text: &str) -> Option<i64> {
    if text.is_empty() {
        return Some(0);
    }
    let negative = text.starts_with('-');
    let digits = text.strip_prefix(['+', '-'])?.trim_start();
    if !digits.bytes().all(|b| b.is_ascii_digit()) || digits.is_empty() {
        return None;
    }
    let value: i64 = digits.parse().ok()?;
    Some(if negative { -value } else { value })
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
                .map(|selector| split(specificity(selector)))
                .max()
                .unwrap_or_default(),
            Simple::Has(arguments) => arguments
                .iter()
                .map(|(_, selector)| split(specificity(selector)))
                .max()
                .unwrap_or_default(),
            // Selectors 4: the pseudo-class plus its most specific `of S` argument.
            Simple::NthOf { of, .. } => {
                let (ids, classes, types) = of
                    .iter()
                    .map(|selector| split(specificity(selector)))
                    .max()
                    .unwrap_or_default();
                (ids, classes + 1, types)
            }
            _ => (0, 1, 0),
        };
        triple = (triple.0 + add.0, triple.1 + add.1, triple.2 + add.2);
    }
    triple
}
/** A scalar specificity back as its (ids, classes, types) triple. */
fn split(n: usize) -> (usize, usize, usize) {
    (n / 1_000_000, n / 1000 % 1000, n % 1000)
}
