/**
 * CSS `text-autospace: normal` (`ideograph-alpha ideograph-numeric`): an eighth of an em goes
 * between an ideograph (Han, kana, bopomofo) and a non-ideographic letter or digit next to it,
 * after Blink's TextAutoSpace. It is applied where text is shaped, as extra advance after the
 * scalar before each boundary, so measuring and painting both carry it.
 */
/// The space inserted at each boundary, as a share of the font size (CSS Text 4: 1/8 em).
const SHARE: f64 = 0.125;
/// `text-autospace: no-autospace`, or upright glyphs, which are never spaced.
pub const OFF: u8 = 0;
/// `text-autospace: normal`.
pub const NORMAL: u8 = 1;
/// `normal` in Chinese text, where UTR #59's conditional characters (ASCII punctuation and
/// symbols) count as narrow and are spaced from ideographs too.
pub const CHINESE: u8 = 2;

/** The mode for text whose `text-autospace` is `on` (and not set upright), in `lang`. */
pub fn mode(on: bool, lang: &str) -> u8 {
    let chinese = lang == "zh" || lang.starts_with("zh-");
    match (on, chinese) {
        (false, _) => OFF,
        (true, false) => NORMAL,
        (true, true) => CHINESE,
    }
}

/** UTR #59's conditional characters, narrow only in Chinese: ASCII punctuation and symbols. */
fn conditional(c: char) -> bool {
    c.is_ascii_punctuation()
}

/** Whether `c` is an ideograph for autospacing: Han, Hiragana, Katakana and Bopomofo letters
 * (punctuation such as U+3002 and U+30FB is not). */
pub fn ideograph(c: char) -> bool {
    matches!(c as u32,
        0x3005..=0x3007 | 0x3021..=0x3029 | 0x3038..=0x303b | 0x3041..=0x3096 | 0x309d..=0x309f
        | 0x30a1..=0x30fa | 0x30fc..=0x30ff | 0x3105..=0x312f | 0x31a0..=0x31bf
        | 0x31f0..=0x31ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
        | 0xff66..=0xff9d | 0x20000..=0x3fffd)
}

/** Whether `c` is a combining mark, which belongs to the letter before it. */
fn mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036f | 0x0591..=0x05c7 | 0x0610..=0x061a | 0x064b..=0x065f
        | 0x0670 | 0x06d6..=0x06ed | 0x1ab0..=0x1aff | 0x1dc0..=0x1dff | 0x20d0..=0x20ff
        | 0xfe00..=0xfe0f | 0xfe20..=0xfe2f | 0xe0100..=0xe01ef)
}

/** Whether `c` is a non-ideographic letter or digit: alphanumeric and not wide (so neither an
 * ideograph, Hangul, nor a fullwidth form). */
fn alpha(c: char) -> bool {
    c.is_alphanumeric()
        && !ideograph(c)
        && !matches!(c as u32, 0x1100..=0x11ff | 0x3130..=0x318f | 0xac00..=0xd7af
            | 0xff00..=0xffef)
}

/** The space autospace puts at a boundary in a font of `size`. */
pub fn space(size: f64) -> f64 {
    size * SHARE
}

/** Whether autospace separates `before` from `after` under `mode` (never when `OFF`). */
pub fn spaced(before: char, after: char, mode: u8) -> bool {
    if mode == OFF {
        return false;
    }
    let narrow = |c: char| alpha(c) || (mode == CHINESE && conditional(c));
    (ideograph(before) && narrow(after)) || (narrow(before) && ideograph(after))
}

/** The UTF-16 ranges (start, length) of the scalars that take the extra space after them, and
 * the space in points for a face of `size`: the last scalar of each cluster before an
 * ideograph-alpha or ideograph-numeric boundary. */
pub fn gaps(text: &str, size: f64, mode: u8) -> (Vec<(usize, usize)>, f64) {
    let mut out = Vec::new();
    let (mut base, mut unit, mut previous) = (None::<char>, 0, None::<(usize, usize)>);
    for c in text.chars() {
        if !mark(c) {
            if let (Some(b), Some(at)) = (base, previous)
                && spaced(b, c, mode)
            {
                out.push(at);
            }
            base = Some(c);
        }
        previous = Some((unit, c.len_utf16()));
        unit += c.len_utf16();
    }
    (out, size * SHARE)
}

#[cfg(test)]
mod tests {
    use super::gaps;

    /** Space goes after the ideograph before a Latin letter and after the letter before one. */
    #[test]
    fn spaces_ideograph_alpha_boundaries() {
        assert_eq!(
            gaps("国国XX国", 40.0, super::NORMAL),
            (vec![(1, 1), (3, 1)], 5.0)
        );
    }

    /** Punctuation is not an ideograph, and a mark stays with its letter. */
    #[test]
    fn skips_punctuation_and_follows_marks() {
        assert_eq!(
            gaps("国。XX", 8.0, super::NORMAL).0,
            Vec::<(usize, usize)>::new()
        );
        assert_eq!(gaps("e\u{301}国", 8.0, super::NORMAL).0, vec![(1, 1)]);
        assert_eq!(gaps("\u{20000}X", 8.0, super::NORMAL).0, vec![(0, 2)]);
    }
}
