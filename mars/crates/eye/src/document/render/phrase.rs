/**
 * `word-break: auto-phrase`: Japanese text breaks between phrases, not between every two
 * characters. Blink segments phrases with BudouX's model; Eye takes its simplest reading, a
 * phrase being a run of kanji or katakana and the hiragana that follow it, so a break falls
 * where hiragana meets kanji or katakana (`東京へ|行きましょう。`, `楽しい|ドライブ。`). A
 * phrase too wide for its line falls back to `normal` breaking inside it, as Blink does.
 */
use super::sizing::LayoutContext;
use super::wrap::{Breaks, MeasuredText};
use crate::{platform::Face, protocol::EngineResult};

/// Overflow a phrase is forgiven before it falls back, one LayoutUnit, as `Flow::fits`.
const FIT_SLACK: f64 = 1.0 / 64.0;

/** Hiragana letters and their marks, which close a phrase. */
fn hiragana(c: char) -> bool {
    matches!(c as u32, 0x3041..=0x309f)
}

/** Kanji (and the iteration mark) and katakana, which open a phrase. */
fn opener(c: char) -> bool {
    matches!(c as u32, 0x3005 | 0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
        | 0x20000..=0x2fa1f | 0x30a1..=0x30fa | 0x30fc..=0x30ff)
}

/** Whether a phrase ends between `before` and `after`. */
pub(super) fn boundary(before: char, after: char) -> bool {
    hiragana(before) && opener(after)
}

/** The line pieces of `word` at `width`, phrase by phrase: a phrase joins the current line when
 * it fits, else starts the next one, and only a phrase wider than a whole line breaks inside, as
 * `normal` text would (then its last piece continues the line). */
pub(super) fn parts(
    word: &str,
    boundaries: &[usize],
    (width, breaks): (f64, Breaks),
    face: Face,
    context: &mut LayoutContext<'_>,
) -> EngineResult<Vec<MeasuredText>> {
    let mut out = Vec::new();
    let mut start = 0;
    for pair in boundaries.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if context.measure(&word[start..to], face)? <= width + FIT_SLACK {
            continue;
        }
        if start < from {
            out.push(piece(word, (start, from), face, context)?);
            start = from;
        }
        if context.measure(&word[from..to], face)? <= width + FIT_SLACK {
            continue;
        }
        let normal = Breaks {
            phrase: false,
            ..breaks
        };
        let mut pieces = super::wrap::lines(&word[from..to], width, face, normal, context)?;
        let last = pieces.pop();
        out.extend(pieces);
        start = to - last.map_or(0, |last| last.text.len());
    }
    if start < word.len() {
        out.push(piece(word, (start, word.len()), face, context)?);
    }
    Ok(out)
}

/** `word[start..end]` measured. */
fn piece(
    word: &str,
    (start, end): (usize, usize),
    face: Face,
    context: &mut LayoutContext<'_>,
) -> EngineResult<MeasuredText> {
    let text = &word[start..end];
    Ok(MeasuredText {
        width: context.measure(text, face)?,
        text: text.to_string(),
    })
}
