/** A monotonic SVG scanner distinguishes command separators, numeric tokens and compact arc flags. */
use super::*;

/** Numeric tokens continue the previous command only where that command permits repetition. */
pub(super) enum Token {
    Command(char),
    Number,
}

/** A comma is permitted only after a completed numeric parameter, never after a command letter. */
pub(super) struct Scanner<'a> {
    rest: &'a str,
    comma: bool,
}
impl<'a> Scanner<'a> {
    /** The borrowed source is bounded by the facade before any scanning starts. */
    pub(super) fn new(rest: &'a str) -> Self {
        Self { rest, comma: false }
    }

    /** SVG whitespace excludes vertical tabs and other ASCII control characters. */
    fn whitespace(&mut self) {
        self.rest = self.rest.trim_start_matches([' ', '\t', '\r', '\n']);
    }

    /** At most one optional comma may occur between numeric parameters. */
    fn separator(&mut self) -> EngineResult<bool> {
        self.whitespace();
        let comma = self.rest.starts_with(',');
        if comma {
            if !self.comma {
                return Err(malformed());
            }
            self.rest = &self.rest[1..];
            self.whitespace();
        }
        Ok(comma)
    }

    /** Commands consume their letter; implicit repetitions leave the first number for the command parser. */
    pub(super) fn command_or_number(&mut self) -> EngineResult<Option<Token>> {
        let comma = self.separator()?;
        let Some(&next) = self.rest.as_bytes().first() else {
            return if comma { Err(malformed()) } else { Ok(None) };
        };
        if next.is_ascii_alphabetic() && !matches!(next, b'e' | b'E') && !comma {
            self.rest = &self.rest[1..];
            self.comma = false;
            return Ok(Some(Token::Command(char::from(next))));
        }
        if next.is_ascii_digit() || matches!(next, b'-' | b'+' | b'.') {
            self.comma = false;
            return Ok(Some(Token::Number));
        }
        Err(malformed())
    }

    /** Flags consume exactly one digit, allowing compact sequences such as 0110. */
    pub(super) fn flag(&mut self) -> EngineResult<bool> {
        self.separator()?;
        let flag = match self.rest.as_bytes().first() {
            Some(b'0') => false,
            Some(b'1') => true,
            _ => return Err(malformed()),
        };
        self.rest = &self.rest[1..];
        self.comma = true;
        Ok(flag)
    }

    /** A number requires a mantissa digit and, when present, at least one exponent digit. */
    pub(super) fn number(&mut self) -> EngineResult<f64> {
        self.separator()?;
        let bytes = self.rest.as_bytes();
        let mut end = usize::from(matches!(bytes.first(), Some(b'-' | b'+')));
        let integer = digits(bytes, &mut end);
        let fraction = if bytes.get(end) == Some(&b'.') {
            end += 1;
            digits(bytes, &mut end)
        } else {
            0
        };
        if integer + fraction == 0 {
            return Err(malformed());
        }
        if matches!(bytes.get(end), Some(b'e' | b'E')) {
            end += 1;
            if matches!(bytes.get(end), Some(b'-' | b'+')) {
                end += 1;
            }
            if digits(bytes, &mut end) == 0 {
                return Err(malformed());
            }
        }
        let value = self.rest[..end].parse::<f64>().map_err(|_| malformed())?;
        if !value.is_finite() {
            return Err(numeric());
        }
        self.rest = &self.rest[end..];
        self.comma = true;
        Ok(value)
    }
}

/** Linear digit consumption cannot rescan input or consume an adjacent sign or decimal point. */
fn digits(bytes: &[u8], end: &mut usize) -> usize {
    let start = *end;
    while bytes.get(*end).is_some_and(u8::is_ascii_digit) {
        *end += 1;
    }
    *end - start
}
