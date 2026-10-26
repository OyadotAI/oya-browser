/**
 * Glyph orientation in vertical text (CSS Writing Modes `text-orientation`): under `mixed`,
 * characters whose Unicode vertical orientation is upright (CJK ideographs, kana, Hangul,
 * fullwidth forms) stand upright and the rest turn with the line; under `upright` every
 * character stands. An upright character advances one em down the line, so measurement and
 * painting both split a run into turned stretches and upright characters the same way.
 * ponytail: the upright set is the main CJK blocks, not UAX #50's full table; extend
 * `stands_upright` with its U and Tu ranges if a page needs more.
 */
/** No upright glyphs: every character turns with the line (horizontal text, `sideways`). */
pub const TURNED: u8 = 0;
/** `text-orientation: mixed` in vertical text: CJK stands upright, the rest turns. */
pub const MIXED: u8 = 1;
/** `text-orientation: upright`: every character stands. */
pub const UPRIGHT: u8 = 2;

/** Unicode blocks whose characters are upright in vertical text (UAX #50 U and Tu, main CJK). */
const UPRIGHT_RANGES: [(u32, u32); 12] = [
    (0x1100, 0x11FF),
    (0x2E80, 0x2FFF),
    (0x3000, 0x303F),
    (0x3040, 0x30FF),
    (0x3100, 0x31FF),
    (0x3200, 0x33FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xA960, 0xA97F),
    (0xAC00, 0xD7FF),
    (0xF900, 0xFAFF),
    (0xFF00, 0xFFEF),
];

/** Zero-width format characters (soft hyphen, zero-width spaces and joiners, bidi controls,
the byte order mark): they take no em of their own, so they never stand. */
fn format(c: char) -> bool {
    matches!(c as u32, 0xAD | 0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x2069 | 0xFEFF)
}

/** Scripts written vertically by nature (Mongolian, Phags-pa): their glyphs keep the line's
turn even under `text-orientation: upright`. */
fn vertical_script(c: char) -> bool {
    matches!(c as u32, 0x1800..=0x18AF | 0x11660..=0x1167F | 0xA840..=0xA87F)
}

/** Whether `c` stands upright under orientation `mode`. */
pub fn stands_upright(c: char, mode: u8) -> bool {
    // A space keeps its own width down the line, between whatever stands around it.
    if format(c) || vertical_script(c) || c == ' ' {
        return false;
    }
    match mode {
        UPRIGHT => true,
        MIXED => UPRIGHT_RANGES
            .iter()
            .any(|&(start, end)| (start..=end).contains(&(c as u32))),
        _ => false,
    }
}

/** A run cut into its stretches in order: (upright, text), each upright character on its own
and the turned characters between them together. */
pub fn segments(text: &str, mode: u8) -> Vec<(bool, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (at, c) in text.char_indices() {
        if !stands_upright(c, mode) {
            continue;
        }
        if start < at {
            out.push((false, &text[start..at]));
        }
        out.push((true, &text[at..at + c.len_utf8()]));
        start = at + c.len_utf8();
    }
    if start < text.len() {
        out.push((false, &text[start..]));
    }
    out
}

/** The advance of `text` along a vertical line: turned stretches by `plain` (their horizontal
width) and each upright character one em plus letter spacing. */
pub fn measure(text: &str, face: super::Face, plain: impl Fn(&str, super::Face) -> f64) -> f64 {
    let flat = super::Face {
        upright: TURNED,
        ..face
    };
    segments(text, face.upright)
        .into_iter()
        .map(|(upright, part)| match upright {
            true => face.size + face.spacing,
            false => plain(part, flat),
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Mixed keeps Latin stretches together and stands each ideograph alone. */
    #[test]
    fn mixed_splits_ideographs_from_latin() {
        let parts = segments("ab\u{6f22}\u{5b57}c", MIXED);
        assert_eq!(
            parts,
            vec![
                (false, "ab"),
                (true, "\u{6f22}"),
                (true, "\u{5b57}"),
                (false, "c")
            ]
        );
    }

    /** Upright stands every character; turned stands none. */
    #[test]
    fn upright_and_turned_are_all_or_nothing() {
        assert_eq!(segments("ab", UPRIGHT), vec![(true, "a"), (true, "b")]);
        assert_eq!(segments("a\u{6f22}", TURNED), vec![(false, "a\u{6f22}")]);
        assert_eq!(
            segments("a\u{ad}", UPRIGHT),
            vec![(true, "a"), (false, "\u{ad}")]
        );
    }
}
