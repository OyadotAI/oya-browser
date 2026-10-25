/**
 * V9's regular expression engine: a parser for ECMAScript patterns (with the web-compatibility
 * leniencies browsers apply) and a backtracking matcher over UTF-16 code units. Every match is
 * bounded by REGEXP_STEPS, so catastrophic patterns fail as a limit instead of hanging the page.
 */
use crate::constants::{REGEXP_GROUPS, REGEXP_NODES, REGEXP_STEPS};

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
    Boundary(bool),
    Group(Box<Node>, Option<usize>),
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
pub(super) fn compile(pattern: &[u16], flags: &str) -> Result<Program, PatternError> {
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
    /** `\p{...}`: general categories approximated with Rust's character predicates. */
    fn property(&mut self, negated: bool) -> Result<Node, PatternError> {
        if !self.eat('{') {
            return Err(PatternError("Invalid property name".into()));
        }
        let mut name = String::new();
        while let Some(c) = self.peek_char() {
            self.position += 1;
            if c == '}' {
                break;
            }
            name.push(c);
        }
        let ranges = property_ranges(&name)
            .ok_or_else(|| PatternError(format!("Invalid property name {name}")))?;
        Ok(Node::Class(ranges, negated))
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

/** Approximate Unicode property sets for common names, built from Rust predicates on the BMP. */
fn property_ranges(name: &str) -> Option<Vec<(u32, u32)>> {
    let name = name
        .strip_prefix("General_Category=")
        .or_else(|| name.strip_prefix("gc="))
        .unwrap_or(name);
    let name = name
        .strip_prefix("Script=")
        .or_else(|| name.strip_prefix("sc="))
        .unwrap_or(name);
    if let Some(ranges) = exact_category(name) {
        return Some(ranges.to_vec());
    }
    let predicate: fn(char) -> bool = match name {
        "L" | "Letter" | "Alphabetic" | "Alpha" => char::is_alphabetic,
        "Lu" | "Uppercase_Letter" | "Uppercase" => char::is_uppercase,
        "Ll" | "Lowercase_Letter" | "Lowercase" => char::is_lowercase,
        "N" | "Number" | "Nd" | "Decimal_Number" => char::is_numeric,
        "White_Space" | "space" => char::is_whitespace,
        "P" | "Punctuation" => |c: char| {
            c.is_ascii_punctuation()
                || ('\u{2010}'..='\u{2027}').contains(&c)
                || ('\u{3000}'..='\u{303f}').contains(&c)
        },
        "Emoji" | "Extended_Pictographic" | "Emoji_Presentation" => |c: char| {
            ('\u{1f000}'..='\u{1faff}').contains(&c) || ('\u{2600}'..='\u{27bf}').contains(&c)
        },
        "Han" => |c: char| {
            ('\u{4e00}'..='\u{9fff}').contains(&c) || ('\u{3400}'..='\u{4dbf}').contains(&c)
        },
        "Latin" => |c: char| c.is_ascii_alphabetic() || ('\u{c0}'..='\u{24f}').contains(&c),
        "Cyrillic" => |c: char| ('\u{400}'..='\u{4ff}').contains(&c),
        "Greek" => |c: char| ('\u{370}'..='\u{3ff}').contains(&c),
        "Arabic" => |c: char| ('\u{600}'..='\u{6ff}').contains(&c),
        "ASCII" => |c: char| c.is_ascii(),
        "Any" => |_| true,
        _ => return None,
    };
    let mut ranges = Vec::new();
    let mut start: Option<u32> = None;
    for code in 0..=PROPERTY_SCAN_END {
        let matches = char::from_u32(code).is_some_and(predicate);
        match (matches, start) {
            (true, None) => start = Some(code),
            (false, Some(low)) => {
                ranges.push((low, code - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(low) = start {
        ranges.push((low, PROPERTY_SCAN_END));
    }
    Some(ranges)
}

/** Space_Separator, exactly as the Unicode Character Database lists it (UnicodeData.txt, Zs). */
const SPACE_SEPARATOR: &[(u32, u32)] = &[
    (0x20, 0x20),
    (0xa0, 0xa0),
    (0x1680, 0x1680),
    (0x2000, 0x200a),
    (0x202f, 0x202f),
    (0x205f, 0x205f),
    (0x3000, 0x3000),
];
/** Line_Separator (Zl). */
const LINE_SEPARATOR: &[(u32, u32)] = &[(0x2028, 0x2028)];
/** Paragraph_Separator (Zp). */
const PARAGRAPH_SEPARATOR: &[(u32, u32)] = &[(0x2029, 0x2029)];
/** Separator (Z): Zs, Zl and Zp. */
const SEPARATOR: &[(u32, u32)] = &[
    (0x20, 0x20),
    (0xa0, 0xa0),
    (0x1680, 0x1680),
    (0x2000, 0x200a),
    (0x2028, 0x2029),
    (0x202f, 0x202f),
    (0x205f, 0x205f),
    (0x3000, 0x3000),
];
/** Control (Cc): the C0 and C1 controls and DEL. */
const CONTROL: &[(u32, u32)] = &[(0x0, 0x1f), (0x7f, 0x9f)];
/** Mark (M): the combining-mark blocks and the marks of the common scripts. */
// ponytail: the main mark runs only, not every scattered mark in UnicodeData.txt; generate the
// full Mn/Mc/Me table from the UCD if a page tests marks in rarer scripts.
const MARK: &[(u32, u32)] = &[
    (0x300, 0x36f),
    (0x483, 0x489),
    (0x591, 0x5bd),
    (0x5bf, 0x5bf),
    (0x5c1, 0x5c2),
    (0x5c4, 0x5c5),
    (0x5c7, 0x5c7),
    (0x610, 0x61a),
    (0x64b, 0x65f),
    (0x670, 0x670),
    (0x6d6, 0x6dc),
    (0x6df, 0x6e4),
    (0x6e7, 0x6e8),
    (0x6ea, 0x6ed),
    (0x900, 0x903),
    (0x93a, 0x93c),
    (0x93e, 0x94f),
    (0x951, 0x957),
    (0x962, 0x963),
    (0xe31, 0xe31),
    (0xe34, 0xe3a),
    (0xe47, 0xe4e),
    (0x1ab0, 0x1aff),
    (0x1dc0, 0x1dff),
    (0x20d0, 0x20f0),
    (0x302a, 0x302f),
    (0x3099, 0x309a),
    (0xfe00, 0xfe0f),
    (0xfe20, 0xfe2f),
];

/** General categories small enough to list exactly, as V8's ICU tables give them. LinkedIn's
 * whitespace trimmer is `/[\t\n\u000B\f\r\u001C-\u001F\p{Zs}\p{Zl}\p{Zp}]/gu`, and the
 * SyntaxError for `Zs` stopped its app modules. */
fn exact_category(name: &str) -> Option<&'static [(u32, u32)]> {
    Some(match name {
        "Zs" | "Space_Separator" => SPACE_SEPARATOR,
        "Zl" | "Line_Separator" => LINE_SEPARATOR,
        "Zp" | "Paragraph_Separator" => PARAGRAPH_SEPARATOR,
        "Z" | "Separator" => SEPARATOR,
        "Cc" | "Control" | "cntrl" => CONTROL,
        "M" | "Mark" | "Combining_Mark" => MARK,
        _ => return None,
    })
}

/** Property sets are computed over the first planes, where page text lives. */
const PROPERTY_SCAN_END: u32 = 0x2ffff;

/** The matcher state for one attempt. */
struct Matcher<'a> {
    program: &'a Program,
    input: &'a [u16],
    captures: Captures,
    steps: usize,
    exceeded: bool,
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
    /** Case-folded comparison. */
    fn same(&self, a: u32, b: u32) -> bool {
        a == b || self.program.ignore_case && fold(a) == fold(b)
    }
    /** Whether a set contains a point. */
    fn in_set(&self, ranges: &[(u32, u32)], point: u32) -> bool {
        let contains = |p: u32| ranges.iter().any(|(low, high)| (*low..=*high).contains(&p));
        contains(point)
            || self.program.ignore_case && (contains(fold(point)) || contains(upper(point)))
    }
    /** Whether a word character sits at a position. */
    fn word_at(&self, position: usize) -> bool {
        self.input.get(position).is_some_and(|unit| {
            WORD.iter()
                .any(|(low, high)| (*low..=*high).contains(&u32::from(*unit)))
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
            Node::Char(expected) => match self.point(position) {
                Some((point, size)) if self.same(point, *expected) => next(self, position + size),
                _ => false,
            },
            Node::Any => match self.point(position) {
                Some((point, size)) if self.program.dot_all || !line_terminator(point) => {
                    next(self, position + size)
                }
                _ => false,
            },
            Node::Class(ranges, negated) => match self.point(position) {
                Some((point, size)) if self.in_set(ranges, point) != *negated => {
                    next(self, position + size)
                }
                _ => false,
            },
            Node::Start => {
                (position == 0
                    || self.program.multiline
                        && line_terminator(u32::from(self.input[position - 1])))
                    && next(self, position)
            }
            Node::End => {
                (position == self.input.len()
                    || self.program.multiline && line_terminator(u32::from(self.input[position])))
                    && next(self, position)
            }
            Node::Boundary(expected) => {
                let before = position > 0 && self.word_at(position - 1);
                let after = self.word_at(position);
                (before != after) == *expected && next(self, position)
            }
            Node::Group(inner, None) => self.node(inner, position, next),
            Node::Group(inner, Some(index)) => {
                let index = *index;
                self.node(inner, position, &mut |matcher: &mut Matcher<'a>, end| {
                    let saved = matcher.captures[index];
                    matcher.captures[index] = Some((position, end));
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
        match nodes.split_first() {
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
        let matched = if ahead {
            self.node(inner, position, &mut |_, _| true)
        } else {
            (0..=position)
                .rev()
                .any(|start| self.node(inner, start, &mut |_, end| end == position))
        };
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
        if position + length > self.input.len() {
            return false;
        }
        for offset in 0..length {
            let a = u32::from(self.input[start + offset]);
            let b = u32::from(self.input[position + offset]);
            if !self.same(a, b) {
                return false;
            }
        }
        next(self, position + length)
    }
}

/** Execute from `start`; sticky programs try only that position. */
pub(super) fn exec(
    program: &Program,
    input: &[u16],
    start: usize,
) -> Result<Option<Captures>, MatchError> {
    let mut steps = 0;
    let last = if program.sticky { start } else { input.len() };
    let mut position = start;
    while position <= last && position <= input.len() {
        let mut matcher = Matcher {
            program,
            input,
            captures: vec![None; program.groups + 1],
            steps,
            exceeded: false,
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

/** Simple case folding to lowercase. */
fn fold(point: u32) -> u32 {
    match char::from_u32(point) {
        Some(c) if c.is_ascii() => c.to_ascii_lowercase() as u32,
        Some(c) => {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(single), None) => single as u32,
                _ => point,
            }
        }
        None => point,
    }
}

/** Simple uppercase mapping. */
fn upper(point: u32) -> u32 {
    match char::from_u32(point) {
        Some(c) => {
            let mut upper = c.to_uppercase();
            match (upper.next(), upper.next()) {
                (Some(single), None) => single as u32,
                _ => point,
            }
        }
        None => point,
    }
}
