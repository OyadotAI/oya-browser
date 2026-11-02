/** CSS Syntax-style scanning: comments, strings, escapes and balanced blocks, never panicking on input. */
/** Comments become a space; strings are copied verbatim so comment openers inside them survive. */
pub(super) fn strip_comments(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => index = string_end(bytes, index),
            b'\\' => index += 2,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                out.push_str(&source[start..index]);
                out.push(' ');
                index = find(bytes, index + 2, b"*/").map_or(bytes.len(), |end| end + 2);
                start = index;
            }
            _ => index += 1,
        }
    }
    out.push_str(source.get(start..).unwrap_or(""));
    out
}
/** Index of the first occurrence of `needle` at or after `from`. */
fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|at| at + from)
}
/** Index just past a string starting at `start`; an unescaped newline or the end closes it. */
fn string_end(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'\n' => return index,
            b if b == quote => return index + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}
/**
 * First byte in `targets` at nesting depth zero from `from`, skipping strings, escapes and
 * balanced `()`, `[]` and `{}`; returns the length when none occurs.
 */
pub(super) fn find_top(source: &str, from: usize, targets: &[u8]) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = from;
    while index < bytes.len() {
        let byte = bytes[index];
        if depth == 0 && targets.contains(&byte) {
            return index;
        }
        match byte {
            b'"' | b'\'' => {
                index = string_end(bytes, index);
                continue;
            }
            b'\\' => index += 1,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    bytes.len()
}
/** Index of the `}` closing the block whose `{` is at `open`, or the length when unclosed. */
pub(super) fn block_end(source: &str, open: usize) -> usize {
    find_top(source, open + 1, b"}")
}
/** Top-level pieces separated by `separator`, trimmed, empty pieces kept out. */
pub(super) fn split_top(source: &str, separator: u8) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    while start <= source.len() {
        let end = find_top(source, start, &[separator]);
        let part = source[start..end].trim();
        if !part.is_empty() {
            parts.push(part);
        }
        start = end + 1;
    }
    parts
}
/** Top-level whitespace-separated tokens, keeping functions such as `rgb(1, 2, 3)` whole. */
pub(super) fn tokens(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    while start <= source.len() {
        let end = find_top(source, start, b" \t\r\n");
        let part = source[start..end].trim();
        if !part.is_empty() {
            parts.push(part);
        }
        start = end + 1;
    }
    parts
}
/** The text of a selector's last compound: after its last top-level combinator or space. */
pub(super) fn split_compounds(selector: &str) -> &str {
    let mut start = 0;
    let mut index = 0;
    while index < selector.len() {
        let end = find_top(selector, index, b" \t\r\n>+~");
        if end < selector.len() {
            start = end + 1;
        }
        index = end + 1;
    }
    &selector[start.min(selector.len())..]
}
