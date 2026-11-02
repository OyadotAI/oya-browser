/** Unicode Default_Ignorable code points (UAX #44), which Blink's ShapeResultSpacing treats as
 * zero-width spaces: they take no `letter-spacing` (Character::TreatAsZeroWidthSpace), so the
 * zero-width joiners, bidi controls and the like a run carries add no gaps of their own. */
/** Whether `c` is default-ignorable, or a carriage return, form feed or object replacement
 * character, which Blink treats the same way. */
pub fn zero_width(c: char) -> bool {
    matches!(c as u32,
        0x0c | 0x0d | 0xad | 0x034f | 0x061c | 0x115f..=0x1160 | 0x17b4..=0x17b5
        | 0x180b..=0x180f | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f | 0x3164
        | 0xfe00..=0xfe0f | 0xfeff | 0xffa0 | 0xfff0..=0xfff8 | 0xfffc | 0x1bca0..=0x1bca3
        | 0x1d173..=0x1d17a | 0xe0000..=0xe0fff)
}

/** The UTF-16 ranges (start, length) of `text`'s zero-width scalars. */
pub fn ranges(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut unit = 0;
    for c in text.chars() {
        if zero_width(c) {
            out.push((unit, c.len_utf16()));
        }
        unit += c.len_utf16();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::ranges;

    /** Joiners and bidi controls are found; letters are not. */
    #[test]
    fn finds_zero_width_controls() {
        assert_eq!(ranges("a\u{200d}b\u{2066}"), vec![(1, 1), (3, 1)]);
    }
}
