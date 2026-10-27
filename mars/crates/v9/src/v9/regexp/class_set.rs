/**
 * v-mode character classes (ECMA-262 22.2.2 ClassSetExpression): nested classes, the set
 * operators `&&` and `--`, `\q{...}` string alternatives, and the early errors that keep them
 * well formed. A class value is its code point ranges plus its strings other than one code point.
 * Under the i flag every leaf is case-closed before any operator runs, so K, k and U+212A behave
 * as one character in `[K--k]`, and strings are keyed by canonical characters.
 */
use std::cmp::Reverse;
use std::collections::BTreeSet;

use super::{
    Node, PatternError, PatternParser, UNSUPPORTED_STRING_PROPERTY, case, class_escape, invert,
};

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

/** The pattern node for a settled set: strings longest first, then the single characters, then
the empty string, so that `[\q{W|}a-c]` matches `a` before it matches nothing. */
fn set_node(set: ClassSet) -> Node {
    if set.strings.is_empty() {
        return Node::Class(set.ranges, false);
    }
    let mut words: Vec<&Vec<u32>> = set.strings.iter().filter(|word| !word.is_empty()).collect();
    words.sort_by_key(|word| Reverse(word.len()));
    let mut alternatives: Vec<Node> = words
        .into_iter()
        .map(|word| Node::Concat(word.iter().map(|point| Node::Char(*point)).collect()))
        .collect();
    alternatives.push(Node::Class(set.ranges, false));
    if set.strings.contains(&Vec::new()) {
        alternatives.push(Node::Empty);
    }
    Node::Alt(alternatives)
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

    /** A nested `[...]` or `[^...]` used as an operand. */
    fn nested_class(&mut self) -> Result<ClassSet, PatternError> {
        self.position += 1;
        let negated = self.eat('^');
        let set = self.class_contents()?;
        self.close_class()?;
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
        match self.property(false) {
            Ok(Node::Class(set, _)) => Ok(self.escape_class(set, negated)),
            Err(error) if negated && error.0.starts_with(UNSUPPORTED_STRING_PROPERTY) => {
                Err(bad_character())
            }
            Err(error) => Err(error),
            Ok(_) => Err(bad_character()),
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
