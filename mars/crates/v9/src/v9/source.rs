/** A bounded UTF-8 cursor reports JavaScript UTF-16 source coordinates. */
use crate::protocol::SourceLocation;

/** The scanner alone owns source advancement and CRLF accounting. */
pub(super) struct Source<'a> {
    source: &'a str,
    location: SourceLocation,
    after_cr: bool,
    /** Page byte offset of the text's start, added to every reported location. */
    base: usize,
}
impl<'a> Source<'a> {
    /** A cursor whose locations are page offsets: the text starts at byte `base` of the page,
    so tokens need no second pass to move them there. Source allocation stays with the caller. */
    pub fn new_at(source: &'a str, base: usize) -> Self {
        Self {
            source,
            location: SourceLocation {
                byte_offset: 0,
                line: 1,
                column: 1,
                script_index: None,
            },
            after_cr: false,
            base,
        }
    }
    /** Remaining bytes always start at a valid UTF-8 boundary. */
    pub fn rest(&self) -> &'a str {
        &self.source[self.location.byte_offset..]
    }
    /** Lookahead has no side effects or hidden source position. */
    pub fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }
    /** Every token records its starting location before consuming source. */
    pub fn location(&self) -> SourceLocation {
        SourceLocation {
            byte_offset: self.location.byte_offset + self.base,
            ..self.location
        }
    }
    /** A whole name of ASCII letters, digits, `$` and `_` with no escape after it, consumed in
    one step: the common identifier, read without decoding a char at a time. None when the
    name starts otherwise or runs into a backslash or non-ASCII character, which the full
    identifier scanner handles. */
    pub fn ascii_name(&mut self) -> Option<&'a str> {
        let rest = self.rest();
        let bytes = rest.as_bytes();
        let part = |b: &u8| b.is_ascii_alphanumeric() || matches!(b, b'$' | b'_');
        if !bytes
            .first()
            .is_some_and(|b| part(b) && !b.is_ascii_digit())
        {
            return None;
        }
        let length = bytes.iter().position(|b| !part(b)).unwrap_or(bytes.len());
        if bytes
            .get(length)
            .is_some_and(|b| *b == b'\\' || !b.is_ascii())
        {
            return None;
        }
        self.location.byte_offset += length;
        self.location.column += length;
        self.after_cr = false;
        Some(&rest[..length])
    }
    /** CRLF is one line; astral characters are two UTF-16 column units. */
    pub fn next(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.location.byte_offset += character.len_utf8();
        if character == '\n' && self.after_cr {
            self.after_cr = false;
            return Some(character);
        }
        if line_terminator(character) {
            self.location.line += 1;
            self.location.column = 1;
        } else {
            self.location.column += character.len_utf16();
        }
        self.after_cr = character == '\r';
        Some(character)
    }
}
/** ECMAScript WhiteSpace and LineTerminator deliberately exclude Unicode NEL. */
pub(super) fn whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\u{b}' | '\u{c}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    ) || line_terminator(c)
}
/** These four source characters participate in JavaScript line-sensitive grammar. */
pub(super) fn line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}
