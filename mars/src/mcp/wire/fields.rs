/** A bounded top-level scan retains ID spelling after the JSON parser has validated all syntax. */
use std::collections::BTreeSet;
/** Detect duplicate envelope keys rather than accepting the JSON map's final overwritten value. */
pub(super) fn inspect(frame: &[u8]) -> (Option<String>, bool, bool) {
    let (mut cursor, mut raw_id, mut duplicate, mut duplicate_id) = (0, None, false, false);
    let mut seen = BTreeSet::new();
    skip_space(frame, &mut cursor);
    cursor += 1;
    loop {
        skip_space(frame, &mut cursor);
        if frame.get(cursor) == Some(&b'}') {
            break;
        }
        let start = cursor;
        cursor = string_end(frame, cursor);
        let key: String =
            serde_json::from_slice(&frame[start..cursor]).expect("validated JSON key");
        skip_space(frame, &mut cursor);
        cursor += 1;
        skip_space(frame, &mut cursor);
        let start = cursor;
        cursor = value_end(frame, cursor);
        if !seen.insert(key.clone()) {
            duplicate = true;
            duplicate_id |= key == "id";
        }
        if key == "id" {
            raw_id = Some(String::from_utf8_lossy(&frame[start..cursor]).trim().into());
        }
        skip_space(frame, &mut cursor);
        if frame.get(cursor) == Some(&b'}') {
            break;
        }
        cursor += 1;
    }
    (raw_id, duplicate, duplicate_id)
}
/** Quoted keys and values honor escaped quotes without decoding untrusted content for diagnostics. */
fn string_end(frame: &[u8], mut cursor: usize) -> usize {
    cursor += 1;
    while let Some(byte) = frame.get(cursor) {
        match byte {
            b'\\' => cursor += 2,
            b'"' => return cursor + 1,
            _ => cursor += 1,
        }
    }
    cursor
}
/** Nested commas and braces belong to one value; only top-level delimiters end a member. */
fn value_end(frame: &[u8], mut cursor: usize) -> usize {
    let mut depth = 0usize;
    while let Some(byte) = frame.get(cursor) {
        match byte {
            b'"' => cursor = string_end(frame, cursor),
            b'[' | b'{' => {
                depth += 1;
                cursor += 1;
            }
            b']' | b'}' if depth > 0 => {
                depth -= 1;
                cursor += 1;
            }
            b',' | b'}' if depth == 0 => break,
            _ => cursor += 1,
        }
    }
    cursor
}
/** JSON whitespace is ASCII; actual embedded string newlines were already rejected by the parser. */
fn skip_space(frame: &[u8], cursor: &mut usize) {
    while frame.get(*cursor).is_some_and(u8::is_ascii_whitespace) {
        *cursor += 1;
    }
}
/** Cancellation retains its original numeric token rather than trusting a rounded nested JSON number. */
pub(super) fn member(frame: &[u8], name: &str) -> Option<String> {
    let (mut cursor, mut found) = (0, None);
    skip_space(frame, &mut cursor);
    if frame.get(cursor) != Some(&b'{') {
        return None;
    }
    cursor += 1;
    loop {
        skip_space(frame, &mut cursor);
        if frame.get(cursor) == Some(&b'}') {
            break;
        }
        let start = cursor;
        cursor = string_end(frame, cursor);
        let key: String = serde_json::from_slice(&frame[start..cursor]).ok()?;
        skip_space(frame, &mut cursor);
        cursor += 1;
        skip_space(frame, &mut cursor);
        let start = cursor;
        cursor = value_end(frame, cursor);
        if key == name {
            if found.is_some() {
                return None;
            }
            found = Some(String::from_utf8_lossy(&frame[start..cursor]).trim().into());
        }
        skip_space(frame, &mut cursor);
        if frame.get(cursor) == Some(&b'}') {
            break;
        }
        cursor += 1;
    }
    found
}
