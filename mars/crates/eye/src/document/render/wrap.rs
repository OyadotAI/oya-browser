/** Bounded scalar-safe line breaking reuses measured widths without rescanning long suffixes. */
use super::sizing::LayoutContext;
use crate::{platform::Face, protocol::EngineResult};

/** Overflow a wrapped line forgives, one Chromium 1/64 px layout unit, matching `Flow::fits`. */
const FIT_SLACK: f64 = 1.0 / 64.0;
/** Placement consumes the exact width from bounded shaping instead of measuring each chunk again. */
#[derive(Clone)]
pub(super) struct MeasuredText {
    /** Owned text retains every scalar admitted from the source word. */
    pub text: String,
    /** Native width measured with the same font and weight as painting. */
    pub width: f64,
}

/** Whitespace-normalized text accumulates widths without remeasuring a growing merged line. */
pub(super) fn lines(
    text: &str,
    width: f64,
    face: Face,
    breakable: bool,
    context: &mut LayoutContext<'_>,
) -> EngineResult<Vec<MeasuredText>> {
    if let Some(lines) = context.wrapped(text, width, face, breakable)? {
        return Ok(lines);
    }
    context.charge(text.chars().count() + 1)?;
    let mut result = Vec::new();
    let mut current = MeasuredText {
        text: String::new(),
        width: 0.0,
    };
    let gap = context.measure(" ", face)?;
    // Only CSS white space separates words; a no-break space stays inside its word.
    let words = text
        .split([' ', '\t', '\n', '\r', '\x0c'])
        .filter(|word| !word.is_empty());
    for word in words {
        let mut chunks = word_parts(word, (width, breakable), face, context)?
            .into_iter()
            .peekable();
        while let Some(chunk) = chunks.next() {
            append_chunk(&mut result, &mut current, chunk, width, gap);
            if chunks.peek().is_some() {
                result.push(std::mem::replace(
                    &mut current,
                    MeasuredText {
                        text: String::new(),
                        width: 0.0,
                    },
                ));
            }
        }
    }
    if !current.text.is_empty() {
        result.push(current);
    }
    context.remember_wrapped(text, width, face, breakable, &result)?;
    Ok(result)
}

/** Combining a measured run copies once; prior line contents are never sent back to the shaper. */
fn append_chunk(
    result: &mut Vec<MeasuredText>,
    current: &mut MeasuredText,
    chunk: MeasuredText,
    width: f64,
    gap: f64,
) {
    if !current.text.is_empty() && current.width + gap + chunk.width <= width + FIT_SLACK {
        current.text.push(' ');
        current.text.push_str(&chunk.text);
        current.width += gap + chunk.width;
        return;
    }
    if !current.text.is_empty() {
        result.push(std::mem::replace(
            current,
            MeasuredText {
                text: String::new(),
                width: 0.0,
            },
        ));
    }
    *current = chunk;
}

/** One boundary table and forward-only indexes avoid repeated whole-suffix scans and shaping. */
fn word_parts(
    word: &str,
    (width, breakable): (f64, bool),
    face: Face,
    context: &mut LayoutContext<'_>,
) -> EngineResult<Vec<MeasuredText>> {
    context.charge(word.chars().count() + 1)?;
    let boundaries = break_offsets(word, breakable);
    let mut start = 0;
    let mut hint = 1;
    let mut result = Vec::new();
    while start + 1 < boundaries.len() {
        let (end, measured) =
            fitting_prefix(word, &boundaries, (start, hint), width, face, context)?;
        result.push(MeasuredText {
            text: word[boundaries[start]..boundaries[end]].into(),
            width: measured,
        });
        hint = end - start;
        start = end;
    }
    Ok(result)
}

/** Previous line length seeds local probes; proportional bracketing avoids restarting every scan at one. */
fn fitting_prefix(
    word: &str,
    boundaries: &[usize],
    position: (usize, usize),
    width: f64,
    face: Face,
    context: &mut LayoutContext<'_>,
) -> EngineResult<(usize, f64)> {
    let (start, hint) = position;
    let last = boundaries.len() - 1;
    let mut fit = (start + hint).min(last);
    let mut measured = context.measure(&word[boundaries[start]..boundaries[fit]], face)?;
    if measured > width {
        let first = start + 1;
        let minimum = if fit == first {
            measured
        } else {
            context.measure(&word[boundaries[start]..boundaries[first]], face)?
        };
        if minimum > width || fit == first {
            return Ok((first, minimum));
        }
        return search_prefix(
            word,
            boundaries,
            start,
            (first, fit, minimum),
            width,
            face,
            context,
        );
    }
    let mut step = 1;
    while fit < last {
        let probe = (fit + step).min(last);
        let candidate = context.measure(&word[boundaries[start]..boundaries[probe]], face)?;
        if candidate > width {
            return search_prefix(
                word,
                boundaries,
                start,
                (fit, probe, measured),
                width,
                face,
                context,
            );
        }
        fit = probe;
        measured = candidate;
        step = step.saturating_mul(2);
    }
    Ok((fit, measured))
}

/** Every binary-search substring is charged through the same context as final placement. */
fn search_prefix(
    word: &str,
    boundaries: &[usize],
    start: usize,
    bracket: (usize, usize, f64),
    width: f64,
    face: Face,
    context: &mut LayoutContext<'_>,
) -> EngineResult<(usize, f64)> {
    let (mut fit, mut overflow, mut measured) = bracket;
    while fit + 1 < overflow {
        let probe = fit + (overflow - fit) / 2;
        let candidate = context.measure(&word[boundaries[start]..boundaries[probe]], face)?;
        if candidate <= width {
            fit = probe;
            measured = candidate;
        } else {
            overflow = probe;
        }
    }
    Ok((fit, measured))
}

/**
 * Byte offsets a space-free run may break at, after Blink's line breaker: by default only UAX #14
 * opportunities inside the run (CJK, after a hyphen or dash between letters, a zero-width space),
 * so a long word overflows its line as in Chromium; `overflow-wrap: break-word | anywhere` or
 * `word-break: break-all` (`breakable`) add every scalar.
 */
// ponytail: owned subset of UAX #14 (ideographs, kana, hangul, fullwidth, the punctuation classes, hyphens, dashes, ZWSP); upgrade to the full pair table when a page needs it.
fn break_offsets(word: &str, breakable: bool) -> Vec<usize> {
    let cjk = word.chars().any(|c| class(c) == Class::Ideographic);
    let chars = word.char_indices().collect::<Vec<_>>();
    let mut offsets = vec![0];
    let mut flags = 0;
    for (index, pair) in chars.windows(2).enumerate() {
        let ((_, before), (offset, after)) = (pair[0], pair[1]);
        let previous = index.checked_sub(1).map(|i| chars[i].1);
        flags = if regional(before) { flags + 1 } else { 0 };
        let opportunity = if cjk {
            ideographic_break(before, after)
        } else {
            hyphen_break(previous, before, after)
        };
        let cluster = joins(before, after) || (regional(after) && flags % 2 == 1);
        if before == '\u{200b}'
            || (!cluster && (breakable || opportunity || breaking_space(before)))
        {
            offsets.push(offset);
        }
    }
    if !word.is_empty() {
        offsets.push(word.len());
    }
    offsets
}
/** UAX #29 grapheme clusters a line never splits, even under `word-break: break-all` (UAX #14
LB9, LB30a): a mark, joiner, variation selector, emoji modifier or tag extends the scalar
before it, and a zero-width joiner glues an emoji sequence (`👨‍💻`) together. */
fn joins(before: char, after: char) -> bool {
    extends(after) || (before == '\u{200d}' && pictographic(after))
}
/** Grapheme Extend and SpacingMark scalars common in text: combining marks, Indic and Thai vowel
signs, Hangul medial and final jamo, joiners, variation selectors, emoji modifiers and tags. */
fn extends(c: char) -> bool {
    matches!(c as u32,
        0x0300..=0x036f | 0x0483..=0x0489 | 0x0591..=0x05bd | 0x05bf | 0x05c1..=0x05c2
        | 0x05c4..=0x05c5 | 0x05c7 | 0x0610..=0x061a | 0x064b..=0x065f | 0x0670
        | 0x06d6..=0x06dc | 0x06df..=0x06e4 | 0x06e7..=0x06e8 | 0x06ea..=0x06ed
        | 0x0900..=0x0903 | 0x093a..=0x094f | 0x0951..=0x0957 | 0x0962..=0x0963
        | 0x0981..=0x0983 | 0x09bc..=0x09d7 | 0x0e31 | 0x0e34..=0x0e3a | 0x0e47..=0x0e4e
        | 0x0eb1 | 0x0eb4..=0x0ebc | 0x0ec8..=0x0ece | 0x1160..=0x11ff | 0x1ab0..=0x1aff
        | 0x1dc0..=0x1dff | 0x200c..=0x200d | 0x20d0..=0x20ff | 0x302a..=0x302f
        | 0x3099..=0x309a | 0xfe00..=0xfe0f | 0xfe20..=0xfe2f | 0x1f3fb..=0x1f3ff
        | 0xe0020..=0xe007f | 0xe0100..=0xe01ef)
}
/** Extended_Pictographic, roughly: the symbol and emoji blocks a ZWJ sequence joins. */
fn pictographic(c: char) -> bool {
    matches!(c as u32, 0x00a9 | 0x00ae | 0x2190..=0x21ff | 0x2300..=0x23ff | 0x2600..=0x27bf
        | 0x2b00..=0x2bff | 0x1f000..=0x1faff)
}
/** Regional indicators pair into flags (UAX #29 GB12, GB13). */
fn regional(c: char) -> bool {
    matches!(c as u32, 0x1f1e6..=0x1f1ff)
}
/** UAX #14 line breaking classes Eye tells apart inside a run. */
#[derive(Clone, Copy, Debug, PartialEq)]
enum Class {
    /** ID (with CJ resolved to ID, as `line-break: normal` does): breaks on either side. */
    Ideographic,
    /** OP: never ends a line (LB14). */
    Open,
    /** CL, CP, EX and IS: never start a line (LB13). */
    Close,
    /** NS and BA: never start a line (LB16, LB21). */
    Nonstarter,
    /** Everything else, letters and symbols alike. */
    Other,
}
/** LB13, LB14, LB16, LB21 and LB31 between two scalars of a run that holds ideographs: a break
needs a CJK scalar on one side (or a closing mark before an opening one, as in `）（`), and
neither a closing mark or nonstarter after nor an opening mark before. */
fn ideographic_break(before: char, after: char) -> bool {
    let (left, right) = (class(before), class(after));
    let between = ideographic(before)
        || ideographic(after)
        || (matches!(left, Class::Close | Class::Nonstarter) && right == Class::Open);
    between && left != Class::Open && !matches!(right, Class::Close | Class::Nonstarter)
}
/** A scalar's class: the punctuation tables first, since several sit in ideographic blocks. */
fn class(c: char) -> Class {
    if OPEN.binary_search(&c).is_ok() {
        Class::Open
    } else if CLOSE.binary_search(&c).is_ok() {
        Class::Close
    } else if NONSTARTER.binary_search(&c).is_ok() {
        Class::Nonstarter
    } else if ideographic(c) {
        Class::Ideographic
    } else {
        Class::Other
    }
}
/** UAX #14 BA spaces that do not collapse in CSS (ogham, the en/em quads and spaces except the
figure space, the medium mathematical space, the ideographic space): a line may break after
each, as Blink's line breaker allows. */
fn breaking_space(c: char) -> bool {
    matches!(c, '\u{1680}' | '\u{2000}'..='\u{2006}' | '\u{2008}'..='\u{200a}' | '\u{205f}' | '\u{3000}')
}
/** UAX #14 LB21 and LB25: a hyphen (HY) or dash (BA, B2) breaks after itself when a letter
precedes it and no digit follows it, so `well-known` may wrap but `-5` and `1-2` stay whole. */
fn hyphen_break(previous: Option<char>, before: char, after: char) -> bool {
    const DASHES: [char; 5] = ['-', '\u{2010}', '\u{2012}', '\u{2013}', '\u{2014}'];
    DASHES.contains(&before)
        && previous.is_some_and(char::is_alphabetic)
        && !after.is_ascii_digit()
        && !DASHES.contains(&after)
}

/** Han, kana, hangul, CJK punctuation and fullwidth forms allow breaks between characters. */
fn ideographic(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x11ff | 0x2e80..=0x303f | 0x3040..=0x30ff | 0x3130..=0x318f | 0x31f0..=0x31ff
        | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xac00..=0xd7af | 0xf900..=0xfaff | 0xff00..=0xffef
        | 0x20000..=0x2fa1f)
}

/** UAX #14 OP (opening punctuation), with the left double quotation mark that CJK text opens with. Sorted for binary search. */
const OPEN: [char; 83] = [
    '\u{0028}', '\u{005b}', '\u{007b}', '\u{00a1}', '\u{00bf}', '\u{0f3a}', '\u{0f3c}', '\u{169b}',
    '\u{201a}', '\u{201c}', '\u{201e}', '\u{2045}', '\u{207d}', '\u{208d}', '\u{2308}', '\u{230a}',
    '\u{2329}', '\u{2768}', '\u{276a}', '\u{276c}', '\u{276e}', '\u{2770}', '\u{2772}', '\u{2774}',
    '\u{27c5}', '\u{27e6}', '\u{27e8}', '\u{27ea}', '\u{27ec}', '\u{27ee}', '\u{2983}', '\u{2985}',
    '\u{2987}', '\u{2989}', '\u{298b}', '\u{298d}', '\u{298f}', '\u{2991}', '\u{2993}', '\u{2995}',
    '\u{2997}', '\u{29d8}', '\u{29da}', '\u{29fc}', '\u{2e18}', '\u{2e22}', '\u{2e24}', '\u{2e26}',
    '\u{2e28}', '\u{2e42}', '\u{2e55}', '\u{2e57}', '\u{2e59}', '\u{2e5b}', '\u{3008}', '\u{300a}',
    '\u{300c}', '\u{300e}', '\u{3010}', '\u{3014}', '\u{3016}', '\u{3018}', '\u{301a}', '\u{301d}',
    '\u{fd3f}', '\u{fe17}', '\u{fe35}', '\u{fe37}', '\u{fe39}', '\u{fe3b}', '\u{fe3d}', '\u{fe3f}',
    '\u{fe41}', '\u{fe43}', '\u{fe47}', '\u{fe59}', '\u{fe5b}', '\u{fe5d}', '\u{ff08}', '\u{ff3b}',
    '\u{ff5b}', '\u{ff5f}', '\u{ff62}',
];
/** UAX #14 CL, CP, EX, IS and SY: closing punctuation, exclamations, infix separators and the solidus. Sorted for binary search. */
const CLOSE: [char; 136] = [
    '\u{0021}', '\u{0029}', '\u{002c}', '\u{002e}', '\u{002f}', '\u{003a}', '\u{003b}', '\u{003f}',
    '\u{005d}', '\u{007d}', '\u{037e}', '\u{0589}', '\u{05c6}', '\u{060c}', '\u{060d}', '\u{061b}',
    '\u{061d}', '\u{061e}', '\u{061f}', '\u{06d4}', '\u{07f8}', '\u{07f9}', '\u{0f0d}', '\u{0f0e}',
    '\u{0f0f}', '\u{0f10}', '\u{0f11}', '\u{0f14}', '\u{0f3b}', '\u{0f3d}', '\u{169c}', '\u{1802}',
    '\u{1803}', '\u{1808}', '\u{1809}', '\u{1944}', '\u{1945}', '\u{2044}', '\u{2046}', '\u{207e}',
    '\u{208e}', '\u{2309}', '\u{230b}', '\u{232a}', '\u{2762}', '\u{2763}', '\u{2769}', '\u{276b}',
    '\u{276d}', '\u{276f}', '\u{2771}', '\u{2773}', '\u{2775}', '\u{27c6}', '\u{27e7}', '\u{27e9}',
    '\u{27eb}', '\u{27ed}', '\u{27ef}', '\u{2984}', '\u{2986}', '\u{2988}', '\u{298a}', '\u{298c}',
    '\u{298e}', '\u{2990}', '\u{2992}', '\u{2994}', '\u{2996}', '\u{2998}', '\u{29d9}', '\u{29db}',
    '\u{29fd}', '\u{2cf9}', '\u{2cfe}', '\u{2e23}', '\u{2e25}', '\u{2e27}', '\u{2e29}', '\u{2e2e}',
    '\u{2e56}', '\u{2e58}', '\u{2e5a}', '\u{2e5c}', '\u{3001}', '\u{3002}', '\u{3009}', '\u{300b}',
    '\u{300d}', '\u{300f}', '\u{3011}', '\u{3015}', '\u{3017}', '\u{3019}', '\u{301b}', '\u{301e}',
    '\u{301f}', '\u{a60e}', '\u{a876}', '\u{a877}', '\u{fd3e}', '\u{fe10}', '\u{fe11}', '\u{fe12}',
    '\u{fe13}', '\u{fe14}', '\u{fe15}', '\u{fe16}', '\u{fe18}', '\u{fe36}', '\u{fe38}', '\u{fe3a}',
    '\u{fe3c}', '\u{fe3e}', '\u{fe40}', '\u{fe42}', '\u{fe44}', '\u{fe48}', '\u{fe50}', '\u{fe52}',
    '\u{fe56}', '\u{fe57}', '\u{fe5a}', '\u{fe5c}', '\u{fe5e}', '\u{ff01}', '\u{ff09}', '\u{ff0c}',
    '\u{ff0e}', '\u{ff1f}', '\u{ff3d}', '\u{ff5d}', '\u{ff60}', '\u{ff61}', '\u{ff63}', '\u{ff64}',
];
/** UAX #14 NS and BA marks seen in CJK and Southeast Asian text, and the IN ellipsis. Sorted for binary search. */
const NONSTARTER: [char; 33] = [
    '\u{0e5a}', '\u{0e5b}', '\u{17d4}', '\u{17d5}', '\u{17d6}', '\u{17d8}', '\u{17da}', '\u{2026}',
    '\u{203c}', '\u{203d}', '\u{2047}', '\u{2048}', '\u{2049}', '\u{3005}', '\u{301c}', '\u{303b}',
    '\u{303c}', '\u{309b}', '\u{309c}', '\u{309d}', '\u{309e}', '\u{30a0}', '\u{30fb}', '\u{30fd}',
    '\u{30fe}', '\u{a015}', '\u{fe54}', '\u{fe55}', '\u{ff1a}', '\u{ff1b}', '\u{ff65}', '\u{ff9e}',
    '\u{ff9f}',
];
