/** Scoped modifier grammar and matcher modes stay local to their actual subexpression. */
use super::{Matcher, Next, Node, PatternError, PatternParser, Program};

/** Only the three grammar-approved flags can change inside a modifier group. */
const IGNORE_CASE: u8 = 1;
const MULTILINE: u8 = 2;
const DOT_ALL: u8 = 4;

/** Immutable effective flags accompany each compiled scoped node; public RegExp flags remain unchanged. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Modes {
    pub(super) ignore_case: bool,
    pub(super) multiline: bool,
    pub(super) dot_all: bool,
}

impl Modes {
    /** Root matching starts with the real compiled pattern flags. */
    pub(super) fn from_program(program: &Program) -> Self {
        Self {
            ignore_case: program.ignore_case,
            multiline: program.multiline,
            dot_all: program.dot_all,
        }
    }

    /** A validated modifier updates exactly one flag, preserving the other effective settings. */
    fn set(&mut self, flag: u8, enabled: bool) {
        match flag {
            IGNORE_CASE => self.ignore_case = enabled,
            MULTILINE => self.multiline = enabled,
            DOT_ALL => self.dot_all = enabled,
            _ => unreachable!("validated modifier flag"),
        }
    }
}

impl PatternParser<'_> {
    /** Parse a noncapturing modifier group after its question mark, restoring lexical modes on every result. */
    pub(super) fn modifier_group(&mut self) -> Result<Node, PatternError> {
        let saved = self.modes;
        let effective = self.modifier_flags()?;
        self.modes = effective;
        let parsed = self.disjunction().and_then(|inner| {
            self.close()?;
            Ok(Node::Scoped(Box::new(inner), effective))
        });
        self.modes = saved;
        parsed
    }

    /** Exact ASCII flags cannot repeat on either side of the single dash; both sides cannot be
    empty. Each fault has V8's own wording. */
    fn modifier_flags(&mut self) -> Result<Modes, PatternError> {
        let mut effective = self.modes;
        let mut seen = 0;
        let mut removing = false;
        loop {
            match self.peek_char() {
                Some(':') if seen != 0 => {
                    self.position += 1;
                    return Ok(effective);
                }
                Some(':') => return Err(invalid_flag_group()),
                Some('-') if removing => {
                    return Err(PatternError("Multiple dashes in flag group".into()));
                }
                Some('-') => {
                    removing = true;
                    self.position += 1;
                }
                Some(character) => {
                    let flag = modifier_flag(character).ok_or_else(invalid_group)?;
                    if seen & flag != 0 {
                        return Err(PatternError("Repeated flag in flag group".into()));
                    }
                    seen |= flag;
                    effective.set(flag, !removing);
                    self.position += 1;
                }
                None => return Err(invalid_group()),
            }
        }
    }
}

/** Source flags are grammar characters, never Unicode case-folded or interpreted as escapes. */
fn modifier_flag(character: char) -> Option<u8> {
    match character {
        'i' => Some(IGNORE_CASE),
        'm' => Some(MULTILINE),
        's' => Some(DOT_ALL),
        _ => None,
    }
}

/** A character that is no modifier flag makes the whole group V8's invalid group. */
fn invalid_group() -> PatternError {
    PatternError("Invalid group".into())
}

/** A modifier group with no flags on either side of its dash. */
fn invalid_flag_group() -> PatternError {
    PatternError("Invalid flag group".into())
}

impl<'a> Matcher<'a> {
    /** Continuations run under outer modes; backtracking reenters the exact inner modes before retrying. */
    pub(super) fn scoped(
        &mut self,
        inner: &Node,
        modes: Modes,
        position: usize,
        next: Next<'_, 'a>,
    ) -> bool {
        let outer = self.modes;
        self.modes = modes;
        let result = self.node(inner, position, &mut |matcher: &mut Matcher<'a>, end| {
            matcher.modes = outer;
            let matched = next(matcher, end);
            matcher.modes = modes;
            matched
        });
        self.modes = outer;
        result
    }
}
