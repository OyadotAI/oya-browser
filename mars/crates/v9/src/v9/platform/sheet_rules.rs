/**
 * Top-level rule boundaries inside a style sheet's text. A sheet keeps its rules as text, so
 * `cssRules`, `insertRule` and `deleteRule` all need to know where each top-level rule starts and
 * ends. A rule ends at the brace that closes its outermost block (so a nested `@media` block
 * stays whole with its rules), or at the semicolon of a statement at-rule such as `@import`.
 * Strings and comments are skipped, so a brace inside `content: "}"` does not end a rule.
 */
use std::ops::Range;

/** Quote characters that open a string in CSS. */
const QUOTES: [u8; 2] = [b'"', b'\''];

/** Byte ranges of the top-level rules of `text`, in source order. An unclosed block at the end
of the text runs to its end, as CSS parsing closes blocks at end of input. */
pub(super) fn top_level_rules(text: &str) -> Vec<Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    let mut start: Option<usize> = None;
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'/' && bytes.get(index + 1) == Some(&b'*') {
            index = comment_end(bytes, index);
            continue;
        }
        if QUOTES.contains(&byte) {
            index = string_end(bytes, index);
            continue;
        }
        match byte {
            b'{' => depth += 1,
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    ranges.extend(start.take().map(|from| from..index + 1));
                }
            }
            b'}' => ranges.extend(start.take().map(|from| from..index + 1)),
            b';' if depth == 0 => {
                ranges.extend(start.take().map(|from| from..index + 1));
            }
            _ if byte.is_ascii_whitespace() || depth > 0 => {}
            _ => {
                start.get_or_insert(index);
            }
        }
        index += 1;
    }
    ranges.extend(start.map(|from| from..text.trim_end().len()));
    ranges
}

/** The index just past the comment that opens at `from`, or the end of the text if unclosed. */
fn comment_end(bytes: &[u8], from: usize) -> usize {
    bytes[from + 2..]
        .windows(2)
        .position(|pair| pair == b"*/")
        .map_or(bytes.len(), |offset| from + 2 + offset + 2)
}

/** The index just past the string that opens at `from`; an escaped quote does not close it. */
fn string_end(bytes: &[u8], from: usize) -> usize {
    let quote = bytes[from];
    let mut index = from + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}
