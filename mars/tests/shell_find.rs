/** Hermetic checks pin find-in-page matching over one painted line. */
#[path = "../src/desktop/window/find_match.rs"]
mod find_match;

use find_match::line_matches;

/// Matching ignores case and finds every occurrence in reading order.
#[test]
fn matches_every_occurrence_case_insensitively() {
    let found = line_matches("Rust and rust", "rust", 0.0, 10.0, 130.0, 16.0);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0], (0.0, 10.0, 40.0, 16.0));
    assert_eq!(found[1], (90.0, 10.0, 40.0, 16.0));
}

/// Boxes are offset from the line's own origin, not the page's.
#[test]
fn boxes_start_at_the_line_origin() {
    let found = line_matches("abcd", "cd", 100.0, 5.0, 40.0, 12.0);
    assert_eq!(found, vec![(120.0, 5.0, 20.0, 12.0)]);
}

/// No occurrence, no boxes.
#[test]
fn absent_text_has_no_matches() {
    assert!(line_matches("hello", "bye", 0.0, 0.0, 50.0, 10.0).is_empty());
}

/// Multibyte text is measured in characters, never in bytes.
#[test]
fn multibyte_text_uses_character_positions() {
    let found = line_matches("éé ab", "ab", 0.0, 0.0, 50.0, 10.0);
    assert_eq!(found, vec![(30.0, 0.0, 20.0, 10.0)]);
}
