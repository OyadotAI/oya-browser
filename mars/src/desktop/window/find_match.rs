/*! Pure text matching for find in page, kept free of AppKit so it can be tested hermetically. */

/** Every occurrence of `needle` in one painted line, as boxes in document coordinates. */
pub fn line_matches(
    text: &str,
    needle: &str,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let haystack = text.to_lowercase();
    let total = haystack.chars().count().max(1) as f64;
    let length = needle.chars().count() as f64;
    // ponytail: positions are proportional to character counts, not measured glyph advances; exact
    // once the display list carries per-glyph offsets.
    haystack
        .match_indices(needle)
        .map(|(start, _)| {
            let before = haystack[..start].chars().count() as f64;
            (
                x + width * before / total,
                y,
                width * length / total,
                height,
            )
        })
        .collect()
}
