/**
 * V9's regular expression engine: a parser for ECMAScript patterns (with the web-compatibility
 * leniencies browsers apply) and a backtracking matcher over UTF-16 code units. Every match is
 * bounded by REGEXP_STEPS, so catastrophic patterns fail as a limit instead of hanging the page.
 */
use crate::constants::{REGEXP_GROUPS, REGEXP_NODES, REGEXP_STEPS};
mod case;
mod case_data;
mod modifiers;
mod unicode;
use modifiers::Modes;
use unicode::property_ranges;

/** A compiled pattern. */
#[derive(Debug)]
pub(super) struct Program {
    root: Node,
    /// Capture groups, not counting the whole match.
    pub groups: usize,
    /// Named groups and their indexes.
    pub names: Vec<(String, usize)>,
    pub global: bool,
    pub ignore_case: bool,
    pub multiline: bool,
    pub dot_all: bool,
    pub unicode: bool,
    pub sticky: bool,
    pub indices: bool,
}

/** A pattern node. */
#[derive(Debug, Clone)]
enum Node {
    Empty,
    Char(u32),
    Any,
    Class(Vec<(u32, u32)>, bool),
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

/** A pattern or flag error, reported to scripts as a SyntaxError. */
#[derive(Debug)]
pub(super) struct PatternError(pub String);

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
        global: false,
        ignore_case: false,
        multiline: false,
        dot_all: false,
        unicode: false,
        sticky: false,
        indices: false,
    };
    for flag in flags.chars() {
        let slot = match flag {
            'g' => &mut program.global,
            'i' => &mut program.ignore_case,
            'm' => &mut program.multiline,
            's' => &mut program.dot_all,
            'u' | 'v' => &mut program.unicode,
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
    let mut parser = PatternParser {
        units: pattern,
        position: 0,
        groups: 0,
        names: Vec::new(),
        nodes: 0,
        unicode: program.unicode,
        modes: Modes::from_program(&program),
        buffer_boundaries,
        depth: 0,
    };
    parser.count_groups();
    let root = parser.disjunction()?;
    if parser.position < pattern.len() {
        return Err(PatternError(
            "Invalid regular expression: unmatched ')'".into(),
        ));
    }
    program.root = root;
    program.groups = parser.groups;
    program.names = parser.names;
    Ok(program)
}

/** Recursive-descent pattern parser. */
struct PatternParser<'a> {
    units: &'a [u16],
    position: usize,
    groups: usize,
    names: Vec<(String, usize)>,
    nodes: usize,
    unicode: bool,
    modes: Modes,
    buffer_boundaries: bool,
    depth: usize,
}

/** Pattern nesting the parser follows before refusing. */
const PATTERN_DEPTH: usize = 256;

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
            return Err(PatternError("Regular expression too large".into()));
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
    /** `a|b|c` */
    fn disjunction(&mut self) -> Result<Node, PatternError> {
        self.depth += 1;
        if self.depth > PATTERN_DEPTH {
            return Err(PatternError("Regular expression too deeply nested".into()));
        }
        let mut alternatives = vec![self.alternative()?];
        while self.eat('|') {
            alternatives.push(self.alternative()?);
        }
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
            terms.push(atom);
        }
        Ok(match terms.len() {
            0 => Node::Empty,
            1 => terms.pop().expect("one term"),
            _ => Node::Concat(terms),
        })
    }
    /** An atom with an optional quantifier. */
    fn term(&mut self) -> Result<Node, PatternError> {
        self.count()?;
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
            Some('{') if self.unicode => {
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
                    return Err(PatternError("Too many capture groups".into()));
                }
                let index = self.groups;
                if self.names.iter().any(|(existing, _)| *existing == name) {
                    return Err(PatternError("Duplicate capture group name".into()));
                }
                self.names.push((name, index));
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
            return Err(PatternError("Too many capture groups".into()));
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
    /** Whether the pattern declares any `(?<name>` group; without one, Annex B reads `\k` as a
    literal `k`. */
    fn has_named_groups(&self) -> bool {
        self.units.windows(3).enumerate().any(|(index, window)| {
            window == [0x28, 0x3f, 0x3c]
                && !matches!(self.units.get(index + 3), Some(0x3d | 0x21))
                && (index == 0 || self.units[index - 1] != 0x5c)
        })
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
        if c == 'k' && (self.unicode || self.has_named_groups()) {
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
        let ranges = property_ranges(&name)
            .ok_or_else(|| PatternError(format!("Invalid property name {name}")))?;
        Ok(Node::Class(
            if negated { invert(&ranges) } else { ranges },
            false,
        ))
    }
    /** `[...]` */
    fn class(&mut self) -> Result<Node, PatternError> {
        self.position += 1;
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
        Ok(ClassAtom::Char(self.character_escape()?))
    }
}

/** A class member: one character or a set. */
enum ClassAtom {
    Char(u32),
    Set(Vec<(u32, u32)>),
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
        if self.steps > REGEXP_STEPS {
            self.exceeded = true;
        }
        if self.exceeded {
            return false;
        }
        match node {
            Node::Empty => next(self, position),
            Node::Char(expected) => match self.atom(position) {
                Some((point, end)) if self.same(point, *expected) => next(self, end),
                _ => false,
            },
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
                let index = self
                    .program
                    .names
                    .iter()
                    .find(|(found, _)| found == name)
                    .map_or(0, |(_, index)| *index);
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
) -> Result<Option<Captures>, MatchError> {
    let mut steps = 0;
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
