/**
 * V9's regular expression engine: a parser for ECMAScript patterns (with the web-compatibility
 * leniencies browsers apply) and a backtracking matcher over UTF-16 code units. Every match is
 * bounded by REGEXP_STEPS, so catastrophic patterns fail as a limit instead of hanging the page.
 */
use crate::constants::{
    REGEXP_DEADLINE_STEPS, REGEXP_DEPTH, REGEXP_GROUPS, REGEXP_NODES, REGEXP_STEPS,
    REGEXP_STEPS_PER_UNIT,
};
mod case;
mod case_data;
mod class_set;
mod group_names;
mod modifiers;
mod unicode;
use group_names::GroupNames;
use modifiers::Modes;
use std::collections::BTreeMap;
use unicode::property_ranges;

/** A compiled pattern. */
#[derive(Debug)]
pub(super) struct Program {
    root: Node,
    /// Capture groups, not counting the whole match.
    pub groups: usize,
    /// Named groups in source order, with every capture index that carries each name.
    pub names: Vec<(String, Vec<usize>)>,
    /// Position in `names` for each name, so back-references by name resolve in O(log n).
    name_slots: BTreeMap<String, usize>,
    pub global: bool,
    pub ignore_case: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub unicode: bool,
    pub sticky: bool,
    pub indices: bool,
}

impl Program {
    /** Every capture index carrying a group name; a duplicated name lists each carrier. */
    pub(super) fn named_indices(&self, name: &str) -> Option<&[usize]> {
        let slot = *self.name_slots.get(name)?;
        self.names.get(slot).map(|(_, indices)| indices.as_slice())
    }
}

/** A pattern node. */
#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(u32),
    /** Adjacent literal atoms share one node and match without recursive callbacks. */
    Literal(Vec<u32>),
    Any,
    Class(Vec<(u32, u32)>, bool),
    /** Finite strings share prefix paths and retain longest-first backtracking. */
    StringSet(Box<class_set::SetMatcher>),
    Start,
    End,
    /** Proposal assertions depend only on buffer boundaries, regardless of multiline mode. */
    BufferStart,
    BufferEnd(bool),
    Boundary(bool),
    Group(Box<Node>, Option<usize>),
    /** Effective i/m/s flags apply only while this subexpression is active. */
    Scoped(Box<Node>, Modes),
    Look(Box<Node>, bool, bool),
    Back(usize),
    NamedBack(String),
    Concat(Vec<Node>),
    Alt(Vec<Node>),
    /// Atom, min, max, greedy, and the capture indexes inside the atom (reset each iteration).
    Repeat(Box<Node>, u32, Option<u32>, bool, (usize, usize)),
}

/** Unicode properties of strings recognized by the v-mode grammar. */
pub(super) const STRING_PROPERTIES: [&str; 7] = [
    "Basic_Emoji",
    "Emoji_Keycap_Sequence",
    "RGI_Emoji",
    "RGI_Emoji_Flag_Sequence",
    "RGI_Emoji_Modifier_Sequence",
    "RGI_Emoji_Tag_Sequence",
    "RGI_Emoji_ZWJ_Sequence",
];

/** The message for a pattern over a size or nesting bound V9 enforces; ECMAScript has no such early error, so a literal carrying one still parses. */
pub(super) const ENGINE_LIMIT: &str = "Regular expression too large for engine limit";

/** A pattern or flag error, reported to scripts as a SyntaxError. */
#[derive(Debug)]
pub(super) struct PatternError(pub String);

impl PatternError {
    /** True when the error is V9's own gap (an unsupported construct or a size bound), not an ECMAScript early error; parse-time checks defer these to the runtime. */
    pub(super) fn is_engine_gap(&self) -> bool {
        self.0.starts_with(ENGINE_LIMIT)
    }
}

/** Why a match stopped without a result. */
#[derive(Debug)]
pub(super) enum MatchError {
    /** REGEXP_STEPS ran out. */
    Limit,
}

/** Captured spans: index 0 is the whole match. */
pub(super) type Captures = Vec<Option<(usize, usize)>>;

/** Parse a pattern with flags. */
pub(super) fn compile(
    pattern: &[u16],
    flags: &str,
    buffer_boundaries: bool,
) -> Result<Program, PatternError> {
    let mut program = Program {
        root: Node::Empty,
        groups: 0,
        names: Vec::new(),
        name_slots: BTreeMap::new(),
        global: false,
        ignore_case: false,
        multiline: false,
        dot_all: false,
        unicode: false,
        sticky: false,
        indices: false,
    };
    let mut unicode_sets = false;
    for flag in flags.chars() {
        let slot = match flag {
            'g' => &mut program.global,
            'i' => &mut program.ignore_case,
            'm' => &mut program.multiline,
            's' => &mut program.dot_all,
            'u' => &mut program.unicode,
            'v' => &mut unicode_sets,
            'y' => &mut program.sticky,
            'd' => &mut program.indices,
            _ => {
                return Err(PatternError(format!(
                    "Invalid regular expression flags '{flags}'"
                )));
            }
        };
        if *slot {
            return Err(PatternError(format!(
                "Invalid regular expression flags '{flags}'"
            )));
        }
        *slot = true;
    }
    if unicode_sets {
        // The v flag implies unicode matching, and it cannot be combined with u.
        if program.unicode {
            return Err(PatternError(format!(
                "Invalid regular expression flags '{flags}'"
            )));
        }
        program.unicode = true;
    }
    let mut parser = PatternParser {
        units: pattern,
        position: 0,
        groups: 0,
        group_names: GroupNames::default(),
        nodes: 0,
        unicode: program.unicode,
        unicode_sets,
        modes: Modes::from_program(&program),
        buffer_boundaries,
        depth: 0,
        named_groups: declares_named_groups(pattern),
        emoji_expansion_limit: false,
    };
    parser.count_groups();
    parser.group_names.open_frame();
    let root = parser.disjunction()?;
    if parser.position < pattern.len() {
        return Err(PatternError(
            "Invalid regular expression: unmatched ')'".into(),
        ));
    }
    if parser.emoji_expansion_limit {
        return Err(PatternError(format!("{ENGINE_LIMIT}: too large")));
    }
    program.root = root;
    program.groups = parser.groups;
    program.names = parser.group_names.entries;
    program.name_slots = parser.group_names.slots;
    Ok(program)
}

/** Recursive-descent pattern parser. */
struct PatternParser<'a> {
    units: &'a [u16],
    position: usize,
    groups: usize,
    group_names: GroupNames,
    nodes: usize,
    unicode: bool,
    /** The v flag: classes use set notation with operators and strings. */
    unicode_sets: bool,
    modes: Modes,
    buffer_boundaries: bool,
    depth: usize,
    /// Whether any `(?<name>` group exists; decided once, since it changes how `\k` reads.
    named_groups: bool,
    /** Oversized sets still finish static validation before compilation is refused. */
    emoji_expansion_limit: bool,
}

/** A property denotes code points or a Unicode set of strings. */
enum Property {
    CodePoints(Vec<(u32, u32)>),
    Strings(String),
}

/** Pattern nesting the parser follows before refusing. */
/** Recursive parser frames remain bounded while supporting V8's 500-level capture case. */
const PATTERN_DEPTH: usize = 512;

impl PatternParser<'_> {
    /** Peek a unit. */
    fn peek(&self) -> Option<u16> {
        self.units.get(self.position).copied()
    }
    /** Peek as a char, for syntax decisions only (values come from `next_point`). A lone
    surrogate is no syntax character, so it reads as U+FFFD rather than ending the pattern. */
    fn peek_char(&self) -> Option<char> {
        self.peek()
            .map(|unit| char::from_u32(u32::from(unit)).unwrap_or(char::REPLACEMENT_CHARACTER))
    }
    /** Consume a unit. */
    fn next(&mut self) -> Option<u16> {
        let unit = self.peek()?;
        self.position += 1;
        Some(unit)
    }
    /** Consume one code point (pairs in unicode mode). */
    fn next_point(&mut self) -> Option<u32> {
        let unit = self.next()?;
        if self.unicode
            && (0xd800..=0xdbff).contains(&unit)
            && let Some(low) = self.peek().filter(|low| (0xdc00..=0xdfff).contains(low))
        {
            self.position += 1;
            return Some(0x10000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(low) - 0xdc00));
        }
        Some(u32::from(unit))
    }
    /** Consume a char if it matches. */
    fn eat(&mut self, expected: char) -> bool {
        if self.peek_char() == Some(expected) {
            self.position += 1;
            true
        } else {
            false
        }
    }
    /** Count a node against the program bound. */
    fn count(&mut self) -> Result<(), PatternError> {
        self.nodes += 1;
        if self.nodes > REGEXP_NODES {
            return Err(PatternError(format!("{ENGINE_LIMIT}: too large")));
        }
        Ok(())
    }
    /** Pre-scan capture groups so forward references are known. */
    fn count_groups(&mut self) {
        let mut index = 0;
        let mut class = false;
        while index < self.units.len() {
            match self.units[index] {
                0x5c => index += 1,
                0x5b => class = true,
                0x5d => class = false,
                0x28 if !class => {
                    let next = self.units.get(index + 1).copied();
                    let after = self.units.get(index + 2).copied();
                    if next != Some(0x3f)
                        || after == Some(0x3c)
                            && !matches!(self.units.get(index + 3), Some(0x3d | 0x21))
                    {
                        self.groups += 0;
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }
    /** `a|b|c`. Each alternative gets its own name scope, so the same group name may appear in
    several alternatives (ES2025) while its carriers still conflict with names outside them. */
    fn disjunction(&mut self) -> Result<Node, PatternError> {
        self.depth += 1;
        if self.depth > PATTERN_DEPTH {
            return Err(PatternError(format!("{ENGINE_LIMIT}: too deeply nested")));
        }
        let mut alternatives = Vec::new();
        let mut declared = Vec::new();
        loop {
            self.group_names.open_frame();
            alternatives.push(self.alternative()?);
            declared.extend(self.group_names.close_frame());
            if !self.eat('|') {
                break;
            }
        }
        self.group_names.adopt(declared);
        self.depth -= 1;
        Ok(if alternatives.len() == 1 {
            alternatives.pop().expect("one alternative")
        } else {
            Node::Alt(alternatives)
        })
    }
    /** A sequence of terms. */
    fn alternative(&mut self) -> Result<Node, PatternError> {
        let mut terms = Vec::new();
        while let Some(c) = self.peek_char() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.term()?;
            self.append_term(&mut terms, atom)?;
        }
        Ok(match terms.len() {
            0 => Node::Empty,
            1 => terms.pop().expect("one term"),
            _ => Node::Concat(terms),
        })
    }
    /** Merge literal runs before charging structural nodes against the bounded regex tree. */
    fn append_term(&mut self, terms: &mut Vec<Node>, atom: Node) -> Result<(), PatternError> {
        match (terms.last_mut(), atom) {
            (Some(Node::Literal(points)), Node::Char(point)) => points.push(point),
            (Some(last @ Node::Char(_)), Node::Char(point)) => {
                let Node::Char(first) = *last else { unreachable!() };
                *last = Node::Literal(vec![first, point]);
            }
            (_, atom) => { self.count()?; terms.push(atom); }
        }
        Ok(())
    }
    /** An atom with an optional quantifier. */
    fn term(&mut self) -> Result<Node, PatternError> {
        let before = self.groups;
        let atom = match self.peek_char() {
            Some('^') => {
                self.position += 1;
                return Ok(Node::Start);
            }
            Some('$') => {
                self.position += 1;
                return Ok(Node::End);
            }
            Some('\\')
                if self.unicode
                    && self.buffer_boundaries
                    && matches!(self.units.get(self.position + 1), Some(0x41 | 0x5a | 0x7a)) =>
            {
                let assertion = self.units[self.position + 1];
                self.position += 2;
                return Ok(if assertion == 0x41 {
                    Node::BufferStart
                } else {
                    Node::BufferEnd(assertion == 0x5a)
                });
            }
            Some('\\') if matches!(self.units.get(self.position + 1), Some(0x62 | 0x42)) => {
                self.position += 2;
                return Ok(Node::Boundary(self.units[self.position - 1] == 0x62));
            }
            Some('(') => self.group()?,
            Some('.') => {
                self.position += 1;
                Node::Any
            }
            Some('[') => self.class()?,
            Some('\\') => self.escape()?,
            Some('*' | '+' | '?') => {
                return Err(PatternError(
                    "Invalid regular expression: nothing to repeat".into(),
                ));
            }
            Some('{') if self.unicode || self.leading_quantifier() => {
                return Err(PatternError(
                    "Invalid regular expression: nothing to repeat".into(),
                ));
            }
            Some(']' | '}') if self.unicode => {
                return Err(PatternError(
                    "Invalid regular expression: lone quantifier brackets".into(),
                ));
            }
            _ => Node::Char(self.next_point().unwrap_or(0)),
        };
        self.quantifier(atom, (before + 1, self.groups + 1))
    }
    /** `*`, `+`, `?`, `{n,m}` and their lazy forms. */
    fn quantifier(&mut self, atom: Node, captures: (usize, usize)) -> Result<Node, PatternError> {
        let (min, max) = match self.peek_char() {
            Some('*') => {
                self.position += 1;
                (0, None)
            }
            Some('+') => {
                self.position += 1;
                (1, None)
            }
            Some('?') => {
                self.position += 1;
                (0, Some(1))
            }
            Some('{') => match self.braces() {
                Some(bounds) => bounds,
                None => return Ok(atom),
            },
            _ => return Ok(atom),
        };
        // Lookbehinds never take a quantifier; lookaheads do only under Annex B (no u flag).
        let unquantifiable = match atom {
            Node::Start | Node::End | Node::Boundary(_) => self.unicode,
            Node::Look(_, ahead, _) => self.unicode || !ahead,
            _ => false,
        };
        if unquantifiable {
            return Err(PatternError(
                "Invalid regular expression: nothing to repeat".into(),
            ));
        }
        let greedy = !self.eat('?');
        if max.is_some_and(|max| max < min) {
            return Err(PatternError(
                "Invalid regular expression: numbers out of order in {} quantifier".into(),
            ));
        }
        Ok(Node::Repeat(Box::new(atom), min, max, greedy, captures))
    }
    /** `{n}`, `{n,}`, `{n,m}`; anything else leaves `{` literal (web compatibility). */
    fn braces(&mut self) -> Option<(u32, Option<u32>)> {
        let start = self.position;
        self.position += 1;
        let min = self.digits();
        let result = match min {
            Some(min) if self.eat('}') => Some((min, Some(min))),
            Some(min) if self.eat(',') => {
                let max = self.digits();
                if self.eat('}') {
                    Some((min, max))
                } else {
                    None
                }
            }
            _ => None,
        };
        if result.is_none() {
            self.position = start;
        }
        result
    }
    /** A complete interval at atom start has nothing to repeat, even in legacy mode. */
    fn leading_quantifier(&mut self) -> bool {
        let start = self.position;
        let result = self.braces().is_some();
        self.position = start;
        result
    }
    /** A decimal number. */
    fn digits(&mut self) -> Option<u32> {
        let start = self.position;
        let mut value: u32 = 0;
        while let Some(digit) = self.peek_char().and_then(|c| c.to_digit(10)) {
            value = value.saturating_mul(10).saturating_add(digit);
            self.position += 1;
        }
        (self.position > start).then_some(value)
    }
    /** Groups, assertions and lookarounds. */
    fn group(&mut self) -> Result<Node, PatternError> {
        self.position += 1;
        if self.eat('?') {
            if matches!(self.peek_char(), Some('i' | 'm' | 's' | '-')) {
                return self.modifier_group();
            }
            if self.eat(':') {
                let inner = self.disjunction()?;
                self.close()?;
                return Ok(Node::Group(Box::new(inner), None));
            }
            if self.eat('=') || self.eat('!') {
                let negative = self.units[self.position - 1] == 0x21;
                let inner = self.disjunction()?;
                self.close()?;
                return Ok(Node::Look(Box::new(inner), true, negative));
            }
            if self.eat('<') {
                if self.eat('=') || self.eat('!') {
                    let negative = self.units[self.position - 1] == 0x21;
                    let inner = self.disjunction()?;
                    self.close()?;
                    return Ok(Node::Look(Box::new(inner), false, negative));
                }
                let name = self.group_name()?;
                self.groups += 1;
                if self.groups > REGEXP_GROUPS {
                    return Err(PatternError(format!(
                        "{ENGINE_LIMIT}: too many capture groups"
                    )));
                }
                let index = self.groups;
                self.group_names.declare(name, index)?;
                let inner = self.disjunction()?;
                self.close()?;
                return Ok(Node::Group(Box::new(inner), Some(index)));
            }
            return Err(PatternError(
                "Invalid regular expression: invalid group".into(),
            ));
        }
        self.groups += 1;
        if self.groups > REGEXP_GROUPS {
            return Err(PatternError(format!(
                "{ENGINE_LIMIT}: too many capture groups"
            )));
        }
        let index = self.groups;
        let inner = self.disjunction()?;
        self.close()?;
        Ok(Node::Group(Box::new(inner), Some(index)))
    }
    /** `)` */
    fn close(&mut self) -> Result<(), PatternError> {
        if self.eat(')') {
            Ok(())
        } else {
            Err(PatternError("Invalid regular expression: missing )".into()))
        }
    }
    /** A group name up to `>`: surrogate pairs and `\u` escapes decode to one code point, and
    the name must be an IdentifierName (approximated with Rust's alphabetic/alphanumeric). */
    fn group_name(&mut self) -> Result<String, PatternError> {
        let invalid = || PatternError("Invalid capture group name".into());
        let mut name = String::new();
        loop {
            let point = match self.peek() {
                None => return Err(invalid()),
                Some(0x3e) => {
                    self.position += 1;
                    break;
                }
                Some(0x5c) => {
                    self.position += 1;
                    if !self.eat('u') {
                        return Err(invalid());
                    }
                    self.name_escape().ok_or_else(invalid)?
                }
                Some(_) => self.name_point(),
            };
            let c = char::from_u32(point).ok_or_else(invalid)?;
            let valid = c == '$'
                || c == '_'
                || if name.is_empty() {
                    c.is_alphabetic()
                } else {
                    c.is_alphanumeric() || c == '\u{200c}' || c == '\u{200d}'
                };
            if !valid {
                return Err(invalid());
            }
            name.push(c);
        }
        if name.is_empty() {
            return Err(invalid());
        }
        Ok(name)
    }
    /** One code point of a group name: a surrogate pair always combines, as in V8. */
    fn name_point(&mut self) -> u32 {
        let unit = u32::from(self.next().unwrap_or(0));
        if (0xd800..=0xdbff).contains(&unit)
            && let Some(low) = self.peek().filter(|low| (0xdc00..=0xdfff).contains(low))
        {
            self.position += 1;
            return 0x10000 + ((unit - 0xd800) << 10) + (u32::from(low) - 0xdc00);
        }
        unit
    }
    /** `\uXXXX`, `\uXXXX\uXXXX` (a pair) or `\u{...}` inside a group name. */
    fn name_escape(&mut self) -> Option<u32> {
        if self.eat('{') {
            let mut value = 0u32;
            while let Some(digit) = self.peek_char().and_then(|c| c.to_digit(16)) {
                value = value.saturating_mul(16).saturating_add(digit);
                self.position += 1;
            }
            return (self.eat('}') && value <= 0x10ffff).then_some(value);
        }
        let high = self.hex(4)?;
        if (0xd800..=0xdbff).contains(&high) {
            let save = self.position;
            if self.eat('\\')
                && self.eat('u')
                && let Some(low) = self.hex(4).filter(|low| (0xdc00..=0xdfff).contains(low))
            {
                return Some(0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00));
            }
            self.position = save;
        }
        Some(high)
    }
    /** An escape outside a class. */
    fn escape(&mut self) -> Result<Node, PatternError> {
        self.position += 1;
        let Some(c) = self.peek_char() else {
            return Err(PatternError("\\ at end of pattern".into()));
        };
        if let Some(set) = class_escape(c) {
            self.position += 1;
            return Ok(set);
        }
        if c.is_ascii_digit() && c != '0' {
            let start = self.position;
            let number = self.digits().unwrap_or(0) as usize;
            if number <= self.groups.max(self.total_groups()) {
                return Ok(Node::Back(number));
            }
            if self.unicode {
                return Err(PatternError("Invalid escape".into()));
            }
            // Annex B: an out-of-range reference is a legacy octal escape or a literal digit.
            self.position = start;
            return Ok(Node::Char(self.legacy_octal()));
        }
        if c == 'k' && (self.unicode || self.named_groups) {
            self.position += 1;
            if !self.eat('<') {
                return Err(PatternError("Invalid named reference".into()));
            }
            let name = self.group_name()?;
            return Ok(Node::NamedBack(name));
        }
        if (c == 'p' || c == 'P') && self.unicode {
            self.position += 1;
            return self.property(c == 'P');
        }
        Ok(Node::Char(self.character_escape()?))
    }
    /** Count every capture group in the pattern (for back-reference validity). */
    fn total_groups(&self) -> usize {
        let mut count = 0;
        let mut index = 0;
        let mut class = false;
        while index < self.units.len() {
            match self.units[index] {
                0x5c => index += 1,
                0x5b => class = true,
                0x5d => class = false,
                0x28 if !class => {
                    let next = self.units.get(index + 1).copied();
                    let after = self.units.get(index + 2).copied();
                    let named = after == Some(0x3c)
                        && !matches!(self.units.get(index + 3), Some(0x3d | 0x21));
                    if next != Some(0x3f) || named {
                        count += 1;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        count
    }
    /** `\0`-style legacy octal escapes. */
    fn legacy_octal(&mut self) -> u32 {
        let mut value = 0;
        let mut digits = 0;
        while digits < 3 {
            match self.peek_char().and_then(|c| c.to_digit(8)) {
                Some(digit) if value * 8 + digit <= 0o377 => {
                    value = value * 8 + digit;
                    self.position += 1;
                    digits += 1;
                }
                _ => break,
            }
        }
        if digits == 0 {
            return u32::from(self.next().unwrap_or(0));
        }
        value
    }
    /** A single-character escape (also used inside classes). */
    fn character_escape(&mut self) -> Result<u32, PatternError> {
        let c = self
            .next_point()
            .ok_or_else(|| PatternError("\\ at end of pattern".into()))?;
        let c = char::from_u32(c).unwrap_or('\u{fffd}');
        Ok(match c {
            'n' => 10,
            'r' => 13,
            't' => 9,
            'v' => 11,
            'f' => 12,
            '0' if !self.peek_char().is_some_and(|d| d.is_ascii_digit()) => 0,
            '0' if self.unicode => return Err(PatternError("Invalid decimal escape".into())),
            '0' => {
                self.position -= 1;
                self.legacy_octal()
            }
            'c' => match self.peek_char() {
                Some(letter) if letter.is_ascii_alphabetic() => {
                    self.position += 1;
                    letter as u32 % 32
                }
                _ if self.unicode => return Err(PatternError("Invalid unicode escape".into())),
                _ => {
                    self.position -= 1;
                    u32::from(b'\\')
                }
            },
            'x' => match self.hex(2) {
                Some(value) => value,
                None if self.unicode => return Err(PatternError("Invalid escape".into())),
                None => 'x' as u32,
            },
            'u' => {
                if self.unicode && self.eat('{') {
                    let digits = self.position;
                    let mut value = 0u32;
                    while let Some(digit) = self.peek_char().and_then(|c| c.to_digit(16)) {
                        value = value.saturating_mul(16).saturating_add(digit);
                        self.position += 1;
                    }
                    if self.position == digits || !self.eat('}') || value > 0x10ffff {
                        return Err(PatternError("Invalid Unicode escape".into()));
                    }
                    value
                } else {
                    match self.hex(4) {
                        Some(high) if self.unicode && (0xd800..=0xdbff).contains(&high) => {
                            let save = self.position;
                            if self.eat('\\')
                                && self.eat('u')
                                && let Some(low) =
                                    self.hex(4).filter(|low| (0xdc00..=0xdfff).contains(low))
                            {
                                0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            } else {
                                self.position = save;
                                high
                            }
                        }
                        Some(value) => value,
                        None if self.unicode => {
                            return Err(PatternError("Invalid Unicode escape".into()));
                        }
                        None => 'u' as u32,
                    }
                }
            }
            // With the u flag only syntax characters and `/` may be identity-escaped.
            other if self.unicode && !"^$\\.*+?()[]{}|/".contains(other) => {
                return Err(PatternError("Invalid escape".into()));
            }
            other => other as u32,
        })
    }
    /** Exactly `count` hex digits, or None (leaving the cursor). */
    fn hex(&mut self, count: usize) -> Option<u32> {
        let start = self.position;
        let mut value = 0;
        for _ in 0..count {
            match self.peek_char().and_then(|c| c.to_digit(16)) {
                Some(digit) => {
                    value = value * 16 + digit;
                    self.position += 1;
                }
                None => {
                    self.position = start;
                    return None;
                }
            }
        }
        Some(value)
    }
    /** Property escapes validate their closing delimiter and complement sets before Unicode case closure. */
    fn property(&mut self, negated: bool) -> Result<Node, PatternError> {
        match self.read_property(negated)? {
            Property::CodePoints(ranges) => {
                Ok(Node::Class(self.complemented(ranges, negated), false))
            }
            Property::Strings(name) => self.emoji_node(&name),
        }
    }
    /** Read a property name before its enclosing set decides how to combine it. */
    fn read_property(&mut self, negated: bool) -> Result<Property, PatternError> {
        if !self.eat('{') {
            return Err(PatternError("Invalid property name".into()));
        }
        let mut name = String::new();
        let mut closed = false;
        while let Some(c) = self.peek_char() {
            self.position += 1;
            if c == '}' {
                closed = true;
                break;
            }
            name.push(c);
        }
        if !closed {
            return Err(PatternError("Invalid property name".into()));
        }
        let ranges = match property_ranges(&name) {
            Some(ranges) => ranges,
            None if STRING_PROPERTIES.contains(&name.as_str()) => {
                if !self.unicode_sets || negated {
                    return Err(PatternError(format!("Invalid property name {name}")));
                }
                return Ok(Property::Strings(name));
            }
            None => return Err(PatternError(format!("Invalid property name {name}"))),
        };
        Ok(Property::CodePoints(ranges))
    }
    /** Under v and i a negated property is the complement of its case closure, so \P{ASCII} excludes K and its Kelvin-sign equivalent alike. */
    fn complemented(&self, ranges: Vec<(u32, u32)>, negated: bool) -> Vec<(u32, u32)> {
        if !negated {
            ranges
        } else if self.unicode_sets && self.modes.ignore_case {
            invert(&case::closed_ranges(&ranges))
        } else {
            invert(&ranges)
        }
    }
    /** `[...]` */
    fn class(&mut self) -> Result<Node, PatternError> {
        self.position += 1;
        if self.unicode_sets {
            return self.class_set_class();
        }
        let negated = self.eat('^');
        let mut ranges = Vec::new();
        loop {
            let Some(c) = self.peek_char() else {
                return Err(PatternError("Invalid regular expression: missing ]".into()));
            };
            if c == ']' {
                self.position += 1;
                break;
            }
            let low = match self.class_atom()? {
                ClassAtom::Set(set) => {
                    if self.unicode && self.class_range_follows() {
                        return Err(PatternError("Invalid character class".into()));
                    }
                    ranges.extend(set);
                    continue;
                }
                ClassAtom::Char(low) => low,
            };
            if self.peek_char() == Some('-')
                && self.units.get(self.position + 1) != Some(&0x5d)
                && self.units.get(self.position + 1).is_some()
            {
                self.position += 1;
                match self.class_atom()? {
                    ClassAtom::Char(high) => {
                        if high < low {
                            return Err(PatternError(
                                "Invalid regular expression: range out of order in character class"
                                    .into(),
                            ));
                        }
                        ranges.push((low, high));
                    }
                    ClassAtom::Set(_) if self.unicode => {
                        return Err(PatternError("Invalid character class".into()));
                    }
                    ClassAtom::Set(set) => {
                        ranges.push((low, low));
                        ranges.push((u32::from(b'-'), u32::from(b'-')));
                        ranges.extend(set);
                    }
                }
            } else {
                ranges.push((low, low));
            }
        }
        Ok(Node::Class(ranges, negated))
    }
    /** A `-` that would make a range with the member just read. */
    fn class_range_follows(&self) -> bool {
        self.peek_char() == Some('-')
            && self
                .units
                .get(self.position + 1)
                .is_some_and(|unit| *unit != 0x5d)
    }
    /** One class member. */
    fn class_atom(&mut self) -> Result<ClassAtom, PatternError> {
        if self.peek_char() != Some('\\') {
            return Ok(ClassAtom::Char(self.next_point().unwrap_or(0)));
        }
        self.position += 1;
        let c = self.peek_char().unwrap_or('\\');
        if let Some(Node::Class(set, negated)) = class_escape(c) {
            self.position += 1;
            let set = if self.unicode && self.modes.ignore_case {
                case::closed_ranges(&set)
            } else {
                set
            };
            return Ok(ClassAtom::Set(if negated { invert(&set) } else { set }));
        }
        if c == 'b' {
            self.position += 1;
            return Ok(ClassAtom::Char(8));
        }
        if c == '-' {
            self.position += 1;
            return Ok(ClassAtom::Char(u32::from(b'-')));
        }
        if (c == 'p' || c == 'P') && self.unicode {
            self.position += 1;
            if let Node::Class(set, negated) = self.property(c == 'P')? {
                return Ok(ClassAtom::Set(if negated { invert(&set) } else { set }));
            }
        }
        if c.is_ascii_digit() && !self.unicode {
            return Ok(ClassAtom::Char(self.legacy_octal()));
        }
        // With named groups present, `\k` is a reference slot and cannot be an identity escape.
        if c == 'k' && self.named_groups {
            return Err(PatternError("Invalid escape".into()));
        }
        Ok(ClassAtom::Char(self.character_escape()?))
    }
}

/** A class member: one character or a set. */
enum ClassAtom {
    Char(u32),
    Set(Vec<(u32, u32)>),
}

/** Whether the pattern declares any `(?<name>` group. Without one, Annex B reads `\k` as a
literal `k`; with one, `\k` must be a named reference, even inside a class. */
fn declares_named_groups(units: &[u16]) -> bool {
    units.windows(3).enumerate().any(|(index, window)| {
        window == [0x28, 0x3f, 0x3c]
            && !matches!(units.get(index + 3), Some(0x3d | 0x21))
            && (index == 0 || units[index - 1] != 0x5c)
    })
}

/** Digits. */
const DIGITS: [(u32, u32); 1] = [(0x30, 0x39)];
/** Word characters. */
const WORD: [(u32, u32); 4] = [(0x30, 0x39), (0x41, 0x5a), (0x5f, 0x5f), (0x61, 0x7a)];
/** ECMAScript whitespace and line terminators. */
const SPACE: [(u32, u32); 10] = [
    (0x09, 0x0d),
    (0x20, 0x20),
    (0xa0, 0xa0),
    (0x1680, 0x1680),
    (0x2000, 0x200a),
    (0x2028, 0x2029),
    (0x202f, 0x202f),
    (0x205f, 0x205f),
    (0x3000, 0x3000),
    (0xfeff, 0xfeff),
];

/** `\d \D \w \W \s \S` as sets. */
fn class_escape(c: char) -> Option<Node> {
    let (set, negated): (&[(u32, u32)], bool) = match c {
        'd' => (&DIGITS, false),
        'D' => (&DIGITS, true),
        'w' => (&WORD, false),
        'W' => (&WORD, true),
        's' => (&SPACE, false),
        'S' => (&SPACE, true),
        _ => return None,
    };
    Some(Node::Class(set.to_vec(), negated))
}

/** The complement of a set over all code points. */
fn invert(set: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut sorted = set.to_vec();
    sorted.sort();
    let mut out = Vec::new();
    let mut next = 0u32;
    for (low, high) in sorted {
        if low > next {
            out.push((next, low - 1));
        }
        next = next.max(high.saturating_add(1));
    }
    if next <= MAX_CODE_POINT {
        out.push((next, MAX_CODE_POINT));
    }
    out
}

/** The largest code point. */
const MAX_CODE_POINT: u32 = 0x10ffff;

/** The matcher state for one attempt. */
struct Matcher<'a> {
    program: &'a Program,
    /** Current scoped flags are restored before the enclosing continuation runs. */
    modes: Modes,
    input: &'a [u16],
    captures: Captures,
    steps: usize,
    /** Steps this search may take: REGEXP_STEPS, or REGEXP_STEPS_PER_UNIT per input unit. */
    budget: usize,
    /** Nested node calls now on the native stack: matching recurses once per repetition, so a
    greedy loop over long text would overflow the stack before running out of steps. */
    depth: usize,
    /** The script deadline, checked every REGEXP_DEADLINE_STEPS steps, so a large budget can
    never outlast the request. */
    deadline: Option<std::time::Instant>,
    exceeded: bool,
    /** Lookbehind evaluates atoms and alternatives from right to left. */
    backward: bool,
}

/** A quantifier's operands, borrowed from its node for the matching recursion. */
struct Repeat<'n> {
    inner: &'n Node,
    min: u32,
    max: Option<u32>,
    greedy: bool,
    captures: (usize, usize),
}

/** A continuation: what must match after the current node. */
type Next<'k, 'a> = &'k mut dyn FnMut(&mut Matcher<'a>, usize) -> bool;

impl<'a> Matcher<'a> {
    /** The code point at a position and its length in units. */
    fn point(&self, position: usize) -> Option<(u32, usize)> {
        let unit = *self.input.get(position)?;
        if self.program.unicode
            && (0xd800..=0xdbff).contains(&unit)
            && let Some(low) = self
                .input
                .get(position + 1)
                .filter(|low| (0xdc00..=0xdfff).contains(*low))
        {
            return Some((
                0x10000 + ((u32::from(unit) - 0xd800) << 10) + (u32::from(*low) - 0xdc00),
                2,
            ));
        }
        Some((u32::from(unit), 1))
    }
    /** Backward atoms decode complete pairs ending at the current position rather than probing forward candidate starts. */
    fn atom(&self, position: usize) -> Option<(u32, usize)> {
        if !self.backward {
            return self
                .point(position)
                .map(|(point, width)| (point, position + width));
        }
        let previous = position.checked_sub(1)?;
        let unit = *self.input.get(previous)?;
        if self.program.unicode
            && (0xdc00..=0xdfff).contains(&unit)
            && previous > 0
            && (0xd800..=0xdbff).contains(&self.input[previous - 1])
        {
            let high = u32::from(self.input[previous - 1]);
            return Some((
                0x10000 + ((high - 0xd800) << 10) + u32::from(unit) - 0xdc00,
                previous - 1,
            ));
        }
        Some((u32::from(unit), previous))
    }
    /** Case-folded comparison. */
    fn same(&self, a: u32, b: u32) -> bool {
        a == b || self.modes.ignore_case && case::same(a, b, self.program.unicode)
    }
    /** Whether a set contains a point. */
    fn in_set(&self, ranges: &[(u32, u32)], point: u32) -> bool {
        let contains = |p: u32| ranges.iter().any(|(low, high)| (*low..=*high).contains(&p));
        contains(point)
            || self.modes.ignore_case
                && case::equivalents(point, self.program.unicode).any(contains)
    }
    /** Traverse one finite set once, then try its accepted endpoints longest first. */
    fn string_set(
        &mut self,
        set: &class_set::SetMatcher,
        position: usize,
        next: Next<'_, 'a>,
    ) -> bool {
        let trie = if self.backward {
            &set.backward
        } else {
            &set.forward
        };
        let (mut node, mut cursor) = (0, position);
        let mut endpoints = Vec::new();
        while let Some((point, end)) = self.atom(cursor) {
            self.steps += 1;
            if self.steps > self.budget || self.expired() {
                self.exceeded = true;
                return false;
            }
            let key = if self.modes.ignore_case {
                case::canonical(point)
            } else {
                point
            };
            let Some(index) = trie[node].edges.get(&key) else {
                break;
            };
            node = *index;
            cursor = end;
            if trie[node].terminal {
                endpoints.push(end);
            }
        }
        for end in endpoints.into_iter().rev() {
            if next(self, end) {
                return true;
            }
            if self.exceeded {
                return false;
            }
        }
        if let Some((point, end)) = self.atom(position)
            && self.in_set(&set.ranges, point)
            && next(self, end)
        {
            return true;
        }
        !self.exceeded && trie[0].terminal && next(self, position)
    }
    /** Whether a word character sits at a position. */
    fn word_at(&self, position: usize) -> bool {
        self.input.get(position).is_some_and(|unit| {
            let point = u32::from(*unit);
            let contains = |p| WORD.iter().any(|(low, high)| (*low..=*high).contains(&p));
            contains(point)
                || self.program.unicode
                    && self.modes.ignore_case
                    && case::equivalents(point, true).any(contains)
        })
    }
    /** Match one node, then the continuation. */
    fn node(&mut self, node: &Node, position: usize, next: Next<'_, 'a>) -> bool {
        self.steps += 1;
        if self.steps > self.budget || self.depth >= REGEXP_DEPTH || self.expired() {
            self.exceeded = true;
        }
        if self.exceeded {
            return false;
        }
        self.depth += 1;
        let matched = self.node_body(node, position, next);
        self.depth -= 1;
        matched
    }
    /** Whether the script deadline passed, read on every REGEXP_DEADLINE_STEPS-th step. */
    fn expired(&self) -> bool {
        self.steps.is_multiple_of(REGEXP_DEADLINE_STEPS)
            && self
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() >= deadline)
    }
    /** `node` once its bounds hold. */
    fn node_body(&mut self, node: &Node, position: usize, next: Next<'_, 'a>) -> bool {
        match node {
            Node::Empty => next(self, position),
            Node::Char(expected) => match self.atom(position) {
                Some((point, end)) if self.same(point, *expected) => next(self, end),
                _ => false,
            },
            Node::Literal(points) => self.literal(points, position, next),
            Node::Any => match self.atom(position) {
                Some((point, end)) if self.modes.dot_all || !line_terminator(point) => {
                    next(self, end)
                }
                _ => false,
            },
            Node::Class(ranges, negated) => match self.atom(position) {
                Some((point, end)) if self.in_set(ranges, point) != *negated => next(self, end),
                _ => false,
            },
            Node::StringSet(set) => self.string_set(set, position, next),
            Node::Start => {
                (position == 0
                    || self.modes.multiline && line_terminator(u32::from(self.input[position - 1])))
                    && next(self, position)
            }
            Node::End => {
                (position == self.input.len()
                    || self.modes.multiline && line_terminator(u32::from(self.input[position])))
                    && next(self, position)
            }
            Node::BufferStart => position == 0 && next(self, position),
            Node::BufferEnd(optional_terminator) => {
                (position == self.input.len()
                    || *optional_terminator && trailing_terminator(self.input, position))
                    && next(self, position)
            }
            Node::Boundary(expected) => {
                let before = position > 0 && self.word_at(position - 1);
                let after = self.word_at(position);
                (before != after) == *expected && next(self, position)
            }
            Node::Group(inner, None) => self.node(inner, position, next),
            Node::Scoped(inner, modes) => self.scoped(inner, *modes, position, next),
            Node::Group(inner, Some(index)) => {
                let index = *index;
                self.node(inner, position, &mut |matcher: &mut Matcher<'a>, end| {
                    let saved = matcher.captures[index];
                    matcher.captures[index] = Some((position.min(end), position.max(end)));
                    if next(matcher, end) {
                        return true;
                    }
                    matcher.captures[index] = saved;
                    false
                })
            }
            Node::Look(inner, ahead, negative) => {
                self.look(inner, *ahead, *negative, position, next)
            }
            Node::Back(index) => self.back(*index, position, next),
            Node::NamedBack(name) => {
                let index = self.named_capture(name);
                self.back(index, position, next)
            }
            Node::Concat(nodes) => self.sequence(nodes, position, next),
            Node::Alt(alternatives) => {
                for alternative in alternatives {
                    if self.node(alternative, position, next) {
                        return true;
                    }
                    if self.exceeded {
                        return false;
                    }
                }
                false
            }
            Node::Repeat(inner, min, max, greedy, captures) => self.repeat(
                &Repeat {
                    inner,
                    min: *min,
                    max: *max,
                    greedy: *greedy,
                    captures: *captures,
                },
                position,
                0,
                next,
            ),
        }
    }
    /** A literal run consumes points in execution order, including reversed lookbehinds. */
    fn literal(&mut self, points: &[u32], position: usize, next: Next<'_, 'a>) -> bool {
        let mut cursor = position;
        for offset in 0..points.len() {
            self.steps += 1;
            if self.steps > self.budget || self.expired() { self.exceeded = true; return false; }
            let index = if self.backward { points.len() - offset - 1 } else { offset };
            let Some((point, end)) = self.atom(cursor) else { return false; };
            if !self.same(point, points[index]) { return false; }
            cursor = end;
        }
        next(self, cursor)
    }
    /** Match nodes in order. */
    fn sequence(&mut self, nodes: &[Node], position: usize, next: Next<'_, 'a>) -> bool {
        let part = if self.backward {
            nodes.split_last()
        } else {
            nodes.split_first()
        };
        match part {
            None => next(self, position),
            Some((first, rest)) => {
                self.node(first, position, &mut |matcher: &mut Matcher<'a>, end| {
                    matcher.sequence(rest, end, next)
                })
            }
        }
    }
    /** Quantified matching; an iteration that consumes nothing ends the loop. */
    fn repeat(&mut self, r: &Repeat<'_>, position: usize, count: u32, next: Next<'_, 'a>) -> bool {
        if count < r.min {
            return self.iterate(r, position, &mut |matcher: &mut Matcher<'a>, end| {
                if end == position && count + 1 >= r.min {
                    return matcher.repeat(r, end, r.min, next);
                }
                matcher.repeat(r, end, count + 1, next)
            });
        }
        let more = r.max.is_none_or(|max| count < max);
        if r.greedy {
            if more
                && self.iterate(r, position, &mut |matcher: &mut Matcher<'a>, end| {
                    end != position && matcher.repeat(r, end, count + 1, next)
                })
            {
                return true;
            }
            !self.exceeded && next(self, position)
        } else {
            if next(self, position) {
                return true;
            }
            more && !self.exceeded
                && self.iterate(r, position, &mut |matcher: &mut Matcher<'a>, end| {
                    end != position && matcher.repeat(r, end, count + 1, next)
                })
        }
    }
    /** One iteration of a quantified atom: RepeatMatcher clears the atom's captures first, so a
    group that does not take part in this iteration reads undefined; they come back on failure. */
    fn iterate(&mut self, r: &Repeat<'_>, position: usize, next: Next<'_, 'a>) -> bool {
        let (first, last) = r.captures;
        if first >= last {
            return self.node(r.inner, position, next);
        }
        let saved = self.captures[first..last].to_vec();
        self.captures[first..last].fill(None);
        if self.node(r.inner, position, next) {
            return true;
        }
        self.captures[first..last].copy_from_slice(&saved);
        false
    }
    /** Lookahead and lookbehind; captures from a successful positive look are kept. */
    fn look(
        &mut self,
        inner: &Node,
        ahead: bool,
        negative: bool,
        position: usize,
        next: Next<'_, 'a>,
    ) -> bool {
        let saved = self.captures.clone();
        let direction = self.backward;
        self.backward = !ahead;
        let matched = self.node(inner, position, &mut |_, _| true);
        self.backward = direction;
        if negative {
            self.captures = saved;
            return !matched && next(self, position);
        }
        if matched && next(self, position) {
            return true;
        }
        self.captures = saved;
        false
    }
    /** The capture a named back-reference reads: among the groups sharing the name, the first one
    that participated (at most one does in any single match path). */
    fn named_capture(&self, name: &str) -> usize {
        let Some(indices) = self.program.named_indices(name) else {
            return 0;
        };
        indices
            .iter()
            .copied()
            .find(|index| matches!(self.captures.get(*index), Some(Some(_))))
            .unwrap_or(indices[0])
    }
    /** A back-reference; an unset group matches empty. */
    fn back(&mut self, index: usize, position: usize, next: Next<'_, 'a>) -> bool {
        let Some(Some((start, end))) = self.captures.get(index).copied() else {
            return next(self, position);
        };
        let length = end - start;
        let Some(target) = (if self.backward {
            position.checked_sub(length)
        } else {
            Some(position)
        }) else {
            return false;
        };
        if target + length > self.input.len()
            || self.program.unicode
                && (inside_pair(self.input, target) || inside_pair(self.input, target + length))
        {
            return false;
        }
        let mut offset = 0;
        while offset < length {
            let Some((a, width)) = self.point(start + offset) else {
                return false;
            };
            let Some((b, other_width)) = self.point(target + offset) else {
                return false;
            };
            if width != other_width || offset + width > length || !self.same(a, b) {
                return false;
            }
            offset += width;
        }
        next(
            self,
            if self.backward {
                target
            } else {
                target + length
            },
        )
    }
}

/** Execute from `start`; sticky programs try only that position. */
pub(super) fn exec(
    program: &Program,
    input: &[u16],
    start: usize,
    deadline: Option<std::time::Instant>,
) -> Result<Option<Captures>, MatchError> {
    let mut steps = 0;
    // Every start position of an unanchored search costs steps, so a fixed budget failed plain
    // scans of long text (YouTube's megabyte page data) and stopped the page's scripts. The
    // budget grows with the text actually searched; catastrophic backtracking on short text
    // still trips it, and the deadline bounds the rest.
    let searched = input.len().saturating_sub(start);
    let budget = REGEXP_STEPS.max(searched.saturating_mul(REGEXP_STEPS_PER_UNIT));
    let last = if program.sticky {
        start
    } else if anchored(&program.root, Modes::from_program(program)) {
        0
    } else {
        input.len()
    };
    let mut position = start;
    if program.unicode
        && position > 0
        && position < input.len()
        && (0xdc00..=0xdfff).contains(&input[position])
        && (0xd800..=0xdbff).contains(&input[position - 1])
    {
        position -= 1;
    }
    while position <= last && position <= input.len() {
        let mut matcher = Matcher {
            program,
            modes: Modes::from_program(program),
            input,
            captures: vec![None; program.groups + 1],
            steps,
            budget,
            depth: 0,
            deadline,
            exceeded: false,
            backward: false,
        };
        let mut end = None;
        let found = matcher.node(&program.root, position, &mut |_, finish| {
            end = Some(finish);
            true
        });
        if matcher.exceeded {
            return Err(MatchError::Limit);
        }
        steps = matcher.steps;
        if found {
            let mut captures = matcher.captures;
            captures[0] = Some((position, end.unwrap_or(position)));
            return Ok(Some(captures));
        }
        position += 1;
        if program.unicode
            && position < input.len()
            && (0xdc00..=0xdfff).contains(&input[position])
            && (0xd800..=0xdbff).contains(&input[position - 1])
        {
            position += 1;
        }
    }
    Ok(None)
}

/** Line terminators for `.`, `^` and `$`. */
fn line_terminator(point: u32) -> bool {
    matches!(point, 0x0a | 0x0d | 0x2028 | 0x2029)
}

/** Only start assertions effective in their local modes can restrict every alternative to offset zero. */
fn anchored(node: &Node, modes: Modes) -> bool {
    match node {
        Node::Start => !modes.multiline,
        Node::BufferStart => true,
        Node::Scoped(inner, inner_modes) => anchored(inner, *inner_modes),
        Node::Group(inner, _) | Node::Look(inner, true, false) => anchored(inner, modes),
        Node::Alt(nodes) => !nodes.is_empty() && nodes.iter().all(|node| anchored(node, modes)),
        Node::Concat(nodes) => nodes.first().is_some_and(|node| anchored(node, modes)),
        _ => false,
    }
}

/** A Unicode matcher never begins or ends a consumed atom between paired UTF-16 surrogates. */
fn inside_pair(input: &[u16], position: usize) -> bool {
    position > 0
        && position < input.len()
        && (0xd800..=0xdbff).contains(&input[position - 1])
        && (0xdc00..=0xdfff).contains(&input[position])
}

/** The optional final terminator can be a single ECMAScript line terminator or a CRLF pair. */
fn trailing_terminator(input: &[u16], position: usize) -> bool {
    input.get(position..).is_some_and(|tail| match tail {
        [unit] => line_terminator(u32::from(*unit)),
        [0x0d, 0x0a] => true,
        _ => false,
    })
}
