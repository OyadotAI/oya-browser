/**
 * v-mode character classes (ECMA-262 22.2.2 ClassSetExpression): nested classes, the set
 * operators `&&` and `--`, `\q{...}` string alternatives, and the early errors that keep them
 * well formed. A class value is its code point ranges plus its strings other than one code point.
 * Under the i flag every leaf is case-closed before any operator runs, so K, k and U+212A behave
 * as one character in `[K--k]`, and strings are keyed by canonical characters.
 */
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use super::{Node, PatternError, PatternParser, Property, case, class_escape, invert};

/** Unicode 17 sequence data generated from Test262; see UPSTREAM.md for provenance. */
const EMOJI_DATA: &str = include_str!("emoji17.txt");
/** Immutable data is parsed once and shared across compiled patterns. */
static EMOJI_SETS: OnceLock<BTreeMap<String, ClassSet>> = OnceLock::new();

/** Parse the checked-in scalar lists into the same set representation as class syntax. */
fn emoji_sets() -> BTreeMap<String, ClassSet> {
    let mut sets = BTreeMap::new();
    let mut current = String::new();
    for line in EMOJI_DATA.lines() {
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            current = name.into();
            sets.insert(current.clone(), ClassSet::default());
        } else if !line.starts_with('#') && !line.is_empty() {
            let word = line
                .split_whitespace()
                .map(|n| u32::from_str_radix(n, 16).expect("generated Unicode scalar"))
                .collect();
            sets.get_mut(&current)
                .expect("generated property header")
                .absorb(word_set(word));
        }
    }
    for set in sets.values_mut() {
        set.maybe_strings = true;
    }
    sets
}

/** Characters that must be escaped to appear as members (ClassSetSyntaxCharacter). */
const SYNTAX: &str = "()[]{}/-|";
/** Punctuators that may not repeat unescaped inside a class (ClassSetReservedDoublePunctuator). */
const DOUBLED: &str = "&!#$%*+,.:;<=>?@^`~";
/** Punctuators that may be escaped to stand for themselves (ClassSetReservedPunctuator). */
const RESERVED: &str = "&-!#%,:;<=>@`~";
/** Escape letters that open a set rather than a single character. */
const SET_ESCAPES: &str = "qdDsSwWpP";
/** The code point `\b` stands for inside a class. */
const BACKSPACE: u32 = 8;
/** Opens the alternative list of `\q`. */
const OPEN_BRACE: char = '{';
/** Separates the alternatives of `\q{...}`. */
const ALTERNATIVE_SEPARATOR: char = '|';

/** A class value: sorted disjoint ranges plus the strings that are not one code point. */
#[derive(Clone, Debug, Default)]
pub(super) struct ClassSet {
    ranges: Vec<(u32, u32)>,
    strings: BTreeSet<Vec<u32>>,
    /** Static MayContainStrings of ECMA-262: true when the syntax can produce a string, even one that
    turns out empty, since a negated class is an early error on that syntax alone. */
    maybe_strings: bool,
}

/** One class member before it joins its enclosing set. */
enum Item {
    /** A nested class, a property, or a class escape. */
    Set(ClassSet),
    /** One character, which may start a range. */
    Char(u32),
}

/** The two binary set operators, which may not be mixed in one class. */
#[derive(Clone, Copy, PartialEq)]
enum Operator {
    Intersect,
    Subtract,
}

impl Operator {
    /** The character the operator doubles, `&` or `-`. */
    fn symbol(self) -> char {
        match self {
            Self::Intersect => '&',
            Self::Subtract => '-',
        }
    }
}

impl ClassSet {
    /** A set holding exactly the given ranges, unsettled. */
    fn from_ranges(ranges: Vec<(u32, u32)>) -> Self {
        Self {
            ranges,
            strings: BTreeSet::new(),
            maybe_strings: false,
        }
    }

    /** The set of one code point. */
    fn point(point: u32) -> Self {
        Self::from_ranges(vec![(point, point)])
    }

    /** Add another set's members without re-sorting; the caller settles once at the end. */
    fn absorb(&mut self, other: ClassSet) {
        self.ranges.extend(other.ranges);
        self.strings.extend(other.strings);
        self.maybe_strings |= other.maybe_strings;
    }
}

/** Sort and merge ranges so that they are disjoint and ascending. */
fn merge(mut ranges: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    ranges.sort_unstable();
    let mut merged: Vec<(u32, u32)> = Vec::with_capacity(ranges.len());
    for (low, high) in ranges {
        match merged.last_mut() {
            Some(last) if low <= last.1.saturating_add(1) => last.1 = last.1.max(high),
            _ => merged.push((low, high)),
        }
    }
    merged
}

/** Two-pointer intersection of two settled range lists, linear in their combined length. */
fn intersect(a: &[(u32, u32)], b: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
        let low = a[i].0.max(b[j].0);
        let high = a[i].1.min(b[j].1);
        if low <= high {
            out.push((low, high));
        }
        if a[i].1 < b[j].1 {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

/** Apply a binary operator to two settled sets. */
fn apply(operator: Operator, a: ClassSet, b: ClassSet) -> ClassSet {
    match operator {
        Operator::Intersect => ClassSet {
            ranges: intersect(&a.ranges, &b.ranges),
            strings: a.strings.intersection(&b.strings).cloned().collect(),
            maybe_strings: a.maybe_strings && b.maybe_strings,
        },
        Operator::Subtract => ClassSet {
            ranges: intersect(&a.ranges, &invert(&b.ranges)),
            strings: a.strings.difference(&b.strings).cloned().collect(),
            maybe_strings: a.maybe_strings,
        },
    }
}

/** A negated class cannot match strings, since its complement would be ambiguous. */
fn complement(set: ClassSet) -> Result<ClassSet, PatternError> {
    if set.maybe_strings {
        return Err(PatternError(
            "Negated character class may contain strings".into(),
        ));
    }
    Ok(ClassSet::from_ranges(invert(&set.ranges)))
}

/** One shared-prefix step of a finite string set. */
#[derive(Clone, Debug, Default)]
pub(super) struct TrieNode {
    pub edges: BTreeMap<u32, usize>,
    pub terminal: bool,
}

/** Forward and backward tries preserve alternatives under lookbehind and backtracking. */
#[derive(Clone, Debug)]
pub(super) struct SetMatcher {
    pub ranges: Vec<(u32, u32)>,
    pub forward: Vec<TrieNode>,
    pub backward: Vec<TrieNode>,
}

/** Insert each sequence once; reverse traversal supports lookbehind. */
fn trie(strings: &BTreeSet<Vec<u32>>, backward: bool) -> Vec<TrieNode> {
    let mut nodes = vec![TrieNode::default()];
    for word in strings {
        let points: Vec<_> = if backward {
            word.iter().rev().copied().collect()
        } else {
            word.clone()
        };
        let mut current = 0;
        for point in points {
            current = if let Some(index) = nodes[current].edges.get(&point) {
                *index
            } else {
                let index = nodes.len();
                nodes.push(TrieNode::default());
                nodes[current].edges.insert(point, index);
                index
            };
        }
        nodes[current].terminal = true;
    }
    nodes
}

/** A settled set matches longer strings first, then characters, then the empty string. */
fn set_node(set: ClassSet) -> Node {
    if set.strings.is_empty() {
        return Node::Class(set.ranges, false);
    }
    Node::StringSet(Box::new(SetMatcher {
        ranges: set.ranges,
        forward: trie(&set.strings, false),
        backward: trie(&set.strings, true),
    }))
}

/** The error for a class that never reaches its closing bracket. */
fn missing_bracket() -> PatternError {
    PatternError("Invalid regular expression: missing ]".into())
}

/** The error for a character that the v-mode grammar does not allow unescaped. */
fn bad_character() -> PatternError {
    PatternError("Invalid character in character class".into())
}

/** The error for a set operator mixed with another operator or with plain members. */
fn bad_operation() -> PatternError {
    PatternError("Invalid set operation in character class".into())
}

/** A `\q` alternative as a set: one character joins the ranges, anything else is a string. */
fn word_set(word: Vec<u32>) -> ClassSet {
    match word.as_slice() {
        [point] => ClassSet::point(*point),
        _ => ClassSet {
            ranges: Vec::new(),
            strings: BTreeSet::from([word]),
            maybe_strings: true,
        },
    }
}

impl PatternParser<'_> {
    /** Charge every generated alternative and scalar before copying it into a pattern. */
    fn emoji_class(&mut self, name: &str) -> Result<ClassSet, PatternError> {
        let set = EMOJI_SETS
            .get_or_init(emoji_sets)
            .get(name)
            .expect("known Unicode property");
        let cost = set.strings.iter().map(|word| word.len() + 1).sum::<usize>() + 2;
        if self.nodes.saturating_add(cost) > super::REGEXP_NODES {
            self.emoji_expansion_limit = true;
            return Ok(ClassSet {
                maybe_strings: true,
                ..ClassSet::default()
            });
        }
        for _ in 0..cost {
            self.count()?;
        }
        Ok(self.settle(set.clone()))
    }

    /** A standalone property uses the same longest-first alternatives as a class operand. */
    pub(super) fn emoji_node(&mut self, name: &str) -> Result<Node, PatternError> {
        Ok(set_node(self.emoji_class(name)?))
    }
    /** The unit `offset` places ahead of the cursor, as a char. */
    fn unit_char(&self, offset: usize) -> Option<char> {
        self.units
            .get(self.position + offset)
            .and_then(|unit| char::from_u32(u32::from(*unit)))
    }

    /** Whether the cursor and the next unit are both `c`. */
    fn doubled_at(&self, c: char) -> bool {
        self.unit_char(0) == Some(c) && self.unit_char(1) == Some(c)
    }

    /** Whether a set operator starts here. */
    fn at_operator(&self, operator: Operator) -> bool {
        self.doubled_at(operator.symbol())
    }

    /** Whether the class body ends here (or the pattern does, which the caller reports). */
    fn at_class_end(&self) -> bool {
        matches!(self.peek_char(), Some(']') | None)
    }

    /** Parse the body of a v-mode class from just after `[`, through its `]`. */
    pub(super) fn class_set_class(&mut self) -> Result<Node, PatternError> {
        self.count()?;
        let negated = self.eat('^');
        let set = self.class_contents()?;
        self.close_class()?;
        let set = if negated { complement(set)? } else { set };
        Ok(set_node(set))
    }

    /** Consume the `]` that closes a class body. */
    fn close_class(&mut self) -> Result<(), PatternError> {
        if self.eat(']') {
            Ok(())
        } else {
            Err(missing_bracket())
        }
    }

    /** The members of a class body: a union, or one operator chain. */
    fn class_contents(&mut self) -> Result<ClassSet, PatternError> {
        if self.at_class_end() {
            return Ok(ClassSet::default());
        }
        let first = self.class_item()?;
        if self.at_operator(Operator::Intersect) {
            return self.operator_chain(first, Operator::Intersect);
        }
        if self.at_operator(Operator::Subtract) {
            return self.operator_chain(first, Operator::Subtract);
        }
        self.union_from(first)
    }

    /** Union of members and ranges up to the class end; a set operator here is an error. */
    fn union_from(&mut self, first: Item) -> Result<ClassSet, PatternError> {
        let mut set = ClassSet::default();
        let mut item = first;
        loop {
            self.add_item(&mut set, item)?;
            if self.at_class_end() {
                return Ok(self.settle(set));
            }
            if self.at_operator(Operator::Intersect) || self.at_operator(Operator::Subtract) {
                return Err(bad_operation());
            }
            item = self.class_item()?;
        }
    }

    /** Merge one member into a union, reading the high end of a range when a `-` follows. */
    fn add_item(&mut self, set: &mut ClassSet, item: Item) -> Result<(), PatternError> {
        match item {
            Item::Set(other) => set.absorb(other),
            Item::Char(low) => {
                let ranged =
                    self.unit_char(0) == Some('-') && !self.at_operator(Operator::Subtract);
                let high = if ranged {
                    self.position += 1;
                    self.range_end(low)?
                } else {
                    low
                };
                set.ranges.push((low, high));
            }
        }
        Ok(())
    }

    /** The high end of a range: one character, not below `low`. */
    fn range_end(&mut self, low: u32) -> Result<u32, PatternError> {
        match self.class_item()? {
            Item::Char(high) if high >= low => Ok(high),
            Item::Char(_) => Err(PatternError(
                "Invalid regular expression: range out of order in character class".into(),
            )),
            Item::Set(_) => Err(PatternError("Invalid character class".into())),
        }
    }

    /** `A && B && C` or `A -- B -- C`: every operand is a settled set of its own. */
    fn operator_chain(
        &mut self,
        first: Item,
        operator: Operator,
    ) -> Result<ClassSet, PatternError> {
        let mut acc = self.operand(first);
        while self.at_operator(operator) {
            self.position += 2;
            if self.unit_char(0) == Some(operator.symbol()) {
                return Err(bad_operation());
            }
            let rhs = self.class_item()?;
            acc = apply(operator, acc, self.operand(rhs));
        }
        if !self.at_class_end() {
            return Err(bad_operation());
        }
        Ok(acc)
    }

    /** An operand as a settled set; a character stands for the set of itself. */
    fn operand(&self, item: Item) -> ClassSet {
        let set = match item {
            Item::Set(set) => set,
            Item::Char(point) => ClassSet::point(point),
        };
        self.settle(set)
    }

    /** One member: a nested class, an escape that opens a set, or a single character. */
    fn class_item(&mut self) -> Result<Item, PatternError> {
        match self.peek_char() {
            Some('[') => Ok(Item::Set(self.nested_class()?)),
            Some('\\') if self.unit_char(1).is_some_and(|c| SET_ESCAPES.contains(c)) => {
                Ok(Item::Set(self.escape_set()?))
            }
            _ => Ok(Item::Char(self.class_set_character()?)),
        }
    }

    /** A nested `[...]` or `[^...]` used as an operand. Nesting shares the pattern depth bound
    with groups and alternatives, so a deeply nested class fails as a limit instead of
    overflowing the native stack; each nested class is also charged against the node budget. */
    fn nested_class(&mut self) -> Result<ClassSet, PatternError> {
        self.count()?;
        self.depth += 1;
        if self.depth > super::PATTERN_DEPTH {
            return Err(PatternError(format!(
                "{}: too deeply nested",
                super::ENGINE_LIMIT
            )));
        }
        self.position += 1;
        let negated = self.eat('^');
        let set = self.class_contents()?;
        self.close_class()?;
        self.depth -= 1;
        if negated { complement(set) } else { Ok(set) }
    }

    /** `\q{...}`, `\d`-style classes, or `\p{...}` / `\P{...}`, at a backslash. */
    fn escape_set(&mut self) -> Result<ClassSet, PatternError> {
        let letter = self.unit_char(1).unwrap_or('\\');
        self.position += 2;
        match letter {
            'q' => self.string_alternatives(),
            'p' | 'P' => self.property_set(letter == 'P'),
            _ => match class_escape(letter) {
                Some(Node::Class(set, negated)) => Ok(self.escape_class(set, negated)),
                _ => Err(bad_character()),
            },
        }
    }

    /** `\p{...}` or `\P{...}`. A property of strings cannot be negated, so `\P` of one is an early error. */
    fn property_set(&mut self, negated: bool) -> Result<ClassSet, PatternError> {
        match self.read_property(negated)? {
            Property::Strings(name) => self.emoji_class(&name),
            Property::CodePoints(set) => Ok(self.escape_class(set, negated)),
        }
    }

    /** A class escape's ranges, closed before complementing so that `[\W]` never gains letters back. */
    fn escape_class(&self, ranges: Vec<(u32, u32)>, negated: bool) -> ClassSet {
        let set = self.settle(ClassSet::from_ranges(ranges));
        if negated {
            ClassSet::from_ranges(invert(&set.ranges))
        } else {
            set
        }
    }

    /** The body of `\q{a|bc|}` after `\q`. */
    fn string_alternatives(&mut self) -> Result<ClassSet, PatternError> {
        if !self.eat(OPEN_BRACE) {
            return Err(bad_character());
        }
        let mut set = ClassSet::default();
        loop {
            let mut word = Vec::new();
            while !matches!(self.peek_char(), Some('|') | Some('}') | None) {
                word.push(self.class_set_character()?);
            }
            set.absorb(word_set(word));
            if self.eat('}') {
                return Ok(self.settle(set));
            }
            if !self.eat(ALTERNATIVE_SEPARATOR) {
                return Err(bad_character());
            }
        }
    }

    /** Canonical form: merged ranges, closed over case under i, and strings keyed canonically. */
    fn settle(&self, set: ClassSet) -> ClassSet {
        if !self.modes.ignore_case {
            return ClassSet {
                ranges: merge(set.ranges),
                ..set
            };
        }
        ClassSet {
            ranges: case::closed_ranges(&set.ranges),
            strings: set
                .strings
                .into_iter()
                .map(|word| word.into_iter().map(case::canonical).collect())
                .collect(),
            maybe_strings: set.maybe_strings,
        }
    }

    /** One ClassSetCharacter: a literal, a reserved-punctuator escape, `\b`, or a character escape. */
    fn class_set_character(&mut self) -> Result<u32, PatternError> {
        let Some(c) = self.peek_char() else {
            return Err(missing_bracket());
        };
        if c == '\\' {
            return self.class_set_escape();
        }
        if SYNTAX.contains(c) || (DOUBLED.contains(c) && self.doubled_at(c)) {
            return Err(bad_character());
        }
        self.next_point().ok_or_else(missing_bracket)
    }

    /** An escape standing for one character, at its backslash. */
    fn class_set_escape(&mut self) -> Result<u32, PatternError> {
        self.position += 1;
        let c = self
            .peek_char()
            .ok_or_else(|| PatternError("\\ at end of pattern".into()))?;
        if RESERVED.contains(c) {
            self.position += 1;
            return Ok(u32::from(c));
        }
        if c == 'b' {
            self.position += 1;
            return Ok(BACKSPACE);
        }
        self.character_escape()
    }
}
